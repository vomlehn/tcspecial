//! The lander's pane: what tcs-sim shows of a world that has a lander, at
//! the left of its window.
//!
//! The landscape and the lander flying over it, drawn to scale, positive X
//! to the right and positive Y up; what it is doing, in words, with a
//! smiling face drawn over it on a landing; the fuel left; its X and Y
//! position and velocity; and the joystick it is flown with.
//!
//! The pictures are path commands in the world's own metres, Y counted down
//! from the top as a picture's is, which the window scales to its size: see
//! `ui/lander.slint`. They are drawn here, from the lander's figures, and the
//! window is brought up to the world every [`REFRESH`].
//!
//! The window watches the world without starting it. The world's clock
//! starts at the first question a simulated payload asks, so that a lander
//! does not fall before its vehicle is there to fly it; a window that asked
//! would start it as soon as it opened. So it reads the world as it stands
//! until then, and the joystick drives it the same way: see
//! `World::glance_component` and `World::drive_by_hand`.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use slint::ComponentHandle;
use tcsprotocol::node::physics::lander::{Point, Resolved};
use tcsprotocol::node::physics::{Place, Resolution};
use tcsprotocol::sim_config::SimConfigFile;

use crate::node::physics::world::World;
use crate::payload::PayloadConfig;
use crate::{LanderProbe, LanderScene, MainWindow, StarClass};

/// The lander's outputs' places, in
/// `tcsprotocol::node::physics::lander::TERMINALS`'s order.
const X: usize = 2;
const Y: usize = 3;
const VELOCITY_X: usize = 4;
const VELOCITY_Y: usize = 5;
const FUEL: usize = 6;
const LANDED: usize = 7;
const CRASHED: usize = 8;
const ACCELERATION_X: usize = 9;
const ACCELERATION_Y: usize = 10;

/// The joystick's inputs' places, in
/// `tcsprotocol::node::physics::joystick::TERMINALS`'s order.
const SET_X: usize = 0;
const SET_Y: usize = 1;

/// The joystick's square's side, which is as thick as each of its
/// rectangles, and how far each rectangle reaches beyond the square, in
/// logical pixels: `ui/lander.slint`'s `Joystick`.
const SQUARE: f32 = 25.2;
const ARM: f32 = 70.0;

/// How often the window is brought up to the world.
const REFRESH: Duration = Duration::from_millis(50);

/// How many stars the sky has.
const STARS: usize = 300;

/// The faintest star, as an apparent magnitude: about what the eye sees on a
/// dark night. The brightest is nought.
const FAINTEST: f64 = 6.0;

/// How many times as many stars there are at each apparent magnitude as at
/// the one brighter: about three, in the night sky.
const MORE_EACH_MAGNITUDE: f64 = 3.0;

/// The apparent magnitude at which a star has shrunk to a single pixel:
/// brighter ones are disks, fainter ones single pixels, dimmer the fainter.
const PIXEL_FROM: f64 = 3.0;

/// The radius of the brightest star's disk, and of a single pixel's, in
/// pixels.
const BRIGHTEST_RADIUS: f64 = 2.0;
const PIXEL_RADIUS: f64 = 0.5;

/// How many classes of brightness the stars are drawn in: half of them
/// disks, from the brightest to a single pixel, and half single pixels,
/// dimming.
const STAR_CLASSES: usize = 8;

/// How wide the landscape is drawn, in logical pixels: `ui/lander.slint`'s
/// scene, which a star's size in pixels is turned into metres by.
const SCENE_WIDTH: f64 = 600.0;

/// How far above the ground a star is at least, as a fraction of the
/// display's height.
const STAR_CLEARANCE: f64 = 0.02;

/// The world's lander, and the joystick it is flown with if there is one.
pub struct Scene {
    /// The world.
    world: Arc<World>,
    /// The lander's place among the world's components.
    lander: usize,
    /// What the model says of it.
    figures: Resolved,
    /// The joystick's place among the world's components.
    joystick: Option<usize>,
}

/// Finds the world's lander, if it has one.
///
/// # Arguments
///
/// * `sim_file` - The simulator file, whose model the world was made from.
/// * `configs`  - The simulated payloads, one of which at least is joined to
///                the world, which is how the world is reached.
///
/// # Returns
///
/// The scene, or `None` where there is no model, it has no lander, or no
/// payload is joined to it.
pub fn scene(
    sim_file: &SimConfigFile,
    configs: &[PayloadConfig],
) -> Option<Scene> {
    let model = sim_file.physics().ok().flatten()?;
    let world = configs
        .iter()
        .find_map(|config| config.attachment.as_ref())?
        .world()
        .clone();
    // The world's components are the model's, in the model's order.
    let (lander, figures) =
        model.components.iter().enumerate().find_map(|(n, c)| {
            match &c.what {
                Resolution::Lander(figures) => Some((n, figures.clone())),
                _ => None,
            }
        })?;
    let joystick = model
        .components
        .iter()
        .position(|c| matches!(c.what, Resolution::Joystick));
    Some(Scene {
        world,
        lander,
        figures,
        joystick,
    })
}

/// A point of the world as a point of the picture: X as it is, Y counted
/// down from the top.
///
/// # Arguments
///
/// * `f`     - The lander's figures, which give the display's height.
/// * `point` - The point, in metres.
///
/// # Returns
///
/// It, as the picture's commands write it.
fn at(f: &Resolved, (x, y): Point) -> String {
    format!("{x:.2} {:.2}", f.height - y)
}

/// The ground and everything below it, filled.
///
/// # Arguments
///
/// * `f` - The lander's figures.
///
/// # Returns
///
/// The picture's commands.
pub fn ground_fill(f: &Resolved) -> String {
    let mut commands = format!("M {}", at(f, (0.0, 0.0)));
    for &point in &f.ground {
        commands.push_str(&format!(" L {}", at(f, point)));
    }
    commands.push_str(&format!(" L {} Z", at(f, (f.width, 0.0))));
    commands
}

/// The ground's line.
///
/// # Arguments
///
/// * `f` - The lander's figures.
///
/// # Returns
///
/// The picture's commands.
pub fn ground_line(f: &Resolved) -> String {
    polyline(f, &f.ground, (0.0, 0.0))
}

/// The pad: its level line, and an upright line at each end, so that it is
/// told from the ground by its shape.
///
/// # Arguments
///
/// * `f` - The lander's figures.
///
/// # Returns
///
/// The picture's commands.
pub fn pad(f: &Resolved) -> String {
    let ((x0, y), (x1, _)) = f.pad_ends();
    let mark = PAD_MARK * f.height;
    format!(
        "M {} L {} M {} L {} M {} L {}",
        at(f, (x0, y)),
        at(f, (x1, y)),
        at(f, (x0, y)),
        at(f, (x0, y + mark)),
        at(f, (x1, y)),
        at(f, (x1, y + mark)),
    )
}

/// How tall each end mark of the pad is, as a fraction of the display's
/// height.
const PAD_MARK: f64 = 1.0 / 50.0;

/// The radius of the light on top of each end mark of the pad, in metres.
const PAD_LIGHT: f64 = 6.0;

/// How long the pad's lights take to flash on and off again, in seconds.
const PAD_FLASH: f64 = 1.0;

/// The lights on top of the pad's two end marks: a round light on each.
///
/// # Arguments
///
/// * `f` - The lander's figures.
///
/// # Returns
///
/// The picture's commands.
pub fn pad_lights(f: &Resolved) -> String {
    let ((x0, y), (x1, _)) = f.pad_ends();
    let top = y + PAD_MARK * f.height + PAD_LIGHT;
    format!(
        "{} {}",
        circle(f, (x0, top), PAD_LIGHT),
        circle(f, (x1, top), PAD_LIGHT)
    )
}

/// Whether the pad's lights are lit at a moment: half of each flash.
///
/// # Arguments
///
/// * `seconds` - The time since the window opened, in seconds.
///
/// # Returns
///
/// True while they are lit.
pub fn pad_lit(seconds: f64) -> bool {
    seconds.rem_euclid(PAD_FLASH) < PAD_FLASH / 2.0
}

/// How far above the highest ground on its way the autopilot keeps the
/// lander's feet, in metres.
const CLEARANCE: f64 = 40.0;

/// The fastest the autopilot takes the lander across, in metres a second.
const CRUISE: f64 = 12.0;

/// How near the pad's middle, across, the lander is to be before the
/// autopilot brings it down, in metres; and how far it may drift once it is
/// coming down before it is taken up and across again.
const OVER_THE_PAD: f64 = 5.0;
const OFF_THE_PAD: f64 = 12.0;

/// How fast across the lander may still be moving when the autopilot starts
/// to bring it down, in metres a second.
const STILL_ENOUGH: f64 = 1.0;

/// The fastest the autopilot brings the lander down, and the speed it
/// touches down at, in metres a second: well within the landing limits.
const DESCENT: f64 = 8.0;
const TOUCHDOWN: f64 = 1.0;

/// How quickly the autopilot closes on the velocity it wants, each second:
/// gentle, since what it asks reaches the thrusters through the vehicles.
const RESPONSE: f64 = 1.0;

/// The autopilot that tries to land the lander, flying it through the
/// joystick as a pilot would: across to above the pad at a safe height, and
/// down onto it. Nothing makes a landing certain: the fuel can run out.
#[derive(Debug, Default)]
pub struct Autopilot {
    /// Whether it has begun to bring the lander down onto the pad.
    descending: bool,
}

impl Autopilot {
    /// What the joystick is to be held at now.
    ///
    /// # Arguments
    ///
    /// * `f`   - The lander's figures.
    /// * `out` - Its terminals' values now, in its type's order.
    ///
    /// # Returns
    ///
    /// The X and Y axes, each from minus one to one.
    pub fn steer(&mut self, f: &Resolved, out: &[f64]) -> (f64, f64) {
        let (x, y) = (out[X], out[Y]);
        let (vx, vy) = (out[VELOCITY_X], out[VELOCITY_Y]);
        let mass = f.dry_mass + out[FUEL].clamp(0.0, 1.0) * f.fuel;
        let across = f.thrust_x / mass;
        let up = f.thrust_y / mass;
        let ((x0, pad), (x1, _)) = f.pad_ends();
        let middle = (x0 + x1) / 2.0;
        let (left, right, _, top) = f.extent();

        // Across: towards the pad's middle, no faster than half the thrust
        // across could stop it in the distance left.
        let to_go = middle - x;
        let speed = (across * to_go.abs())
            .sqrt()
            .min(0.4 * to_go.abs())
            .min(CRUISE);
        let want_vx = speed.copysign(to_go);
        let throttle_x = RESPONSE * (want_vx - vx) / across;

        // Up and down: over the pad and nearly still across, down onto it;
        // anywhere else, up to a height clear of the ground between here
        // and the pad.
        if self.descending {
            self.descending = to_go.abs() < OFF_THE_PAD;
        } else {
            self.descending =
                to_go.abs() < OVER_THE_PAD && vx.abs() < STILL_ENOUGH;
        }
        let want_vy = if self.descending {
            let height = (y - pad).max(0.0);
            let can_stop =
                (2.0 * 0.5 * (up - f.gravity).max(0.0) * height).sqrt();
            -(0.25 * height)
                .clamp(TOUCHDOWN, DESCENT)
                .min(can_stop.max(TOUCHDOWN))
        } else {
            let lowest = x.min(middle) + left - OFF_THE_PAD;
            let highest = x.max(middle) + right + OFF_THE_PAD;
            let safe = (highest_ground(f, lowest, highest) + CLEARANCE)
                .min(f.height - top);
            (0.4 * (safe - y)).clamp(-DESCENT, DESCENT)
        };
        let throttle_y = (RESPONSE * (want_vy - vy) + f.gravity) / up;
        (throttle_x.clamp(-1.0, 1.0), throttle_y.clamp(-1.0, 1.0))
    }
}

/// The highest the ground is between two places across the display.
///
/// # Arguments
///
/// * `f`    - The lander's figures.
/// * `from` - The leftmost place, in metres.
/// * `to`   - The rightmost.
///
/// # Returns
///
/// The height, in metres.
fn highest_ground(f: &Resolved, from: f64, to: f64) -> f64 {
    f.ground
        .iter()
        .filter(|(x, _)| (from..=to).contains(x))
        .map(|&(_, y)| y)
        .fold(ground_at(f, from).max(ground_at(f, to)), f64::max)
}

/// Where the joystick's dot is drawn for an axis, from the square's middle,
/// in logical pixels: the inverse of [`axes`] along one axis.
///
/// # Arguments
///
/// * `axis` - The axis's value, minus one to one.
///
/// # Returns
///
/// The offset: nought for nought, and the arm's end for one.
pub fn dot_of(axis: f64) -> f32 {
    if axis == 0.0 {
        return 0.0;
    }
    let along = SQUARE / 2.0 + (axis.abs().min(1.0) as f32) * ARM;
    along.copysign(axis as f32)
}

/// A line of points, moved by an offset.
///
/// # Arguments
///
/// * `f`      - The lander's figures.
/// * `points` - The points.
/// * `by`     - The offset, in metres.
///
/// # Returns
///
/// The picture's commands.
fn polyline(f: &Resolved, points: &[Point], by: Point) -> String {
    let mut commands = String::new();
    for (n, &(x, y)) in points.iter().enumerate() {
        let verb = if n == 0 { "M" } else { "L" };
        if !commands.is_empty() {
            commands.push(' ');
        }
        commands.push_str(&format!("{verb} {}", at(f, (x + by.0, y + by.1))));
    }
    commands
}

/// The ground's height at a place across the display.
///
/// # Arguments
///
/// * `f` - The lander's figures.
/// * `x` - The place, in metres.
///
/// # Returns
///
/// The height, in metres, along the line between the ground's points
/// either side.
fn ground_at(f: &Resolved, x: f64) -> f64 {
    for pair in f.ground.windows(2) {
        let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
        if x <= x1 {
            return y0 + (y1 - y0) * (x - x0) / (x1 - x0);
        }
    }
    f.ground.last().map_or(0.0, |&(_, y)| y)
}

/// An apparent magnitude drawn at random, as the night sky's are: each
/// magnitude [`MORE_EACH_MAGNITUDE`] times as common as the one brighter,
/// from nought to [`FAINTEST`].
///
/// # Arguments
///
/// * `u` - A number drawn evenly from nought to one.
///
/// # Returns
///
/// The magnitude.
pub fn magnitude(u: f64) -> f64 {
    let k = MORE_EACH_MAGNITUDE;
    (1.0 + u * (k.powf(FAINTEST) - 1.0)).ln() / k.ln()
}

/// The class a star of an apparent magnitude is drawn in, and how that
/// class is drawn.
///
/// # Arguments
///
/// * `m` - The magnitude.
///
/// # Returns
///
/// The class, from nought, the brightest; its radius, in pixels; and its
/// brightness, from nought to one.
pub fn star_class(m: f64) -> (usize, f64, f32) {
    let width = FAINTEST / STAR_CLASSES as f64;
    let class = ((m / width) as usize).min(STAR_CLASSES - 1);
    let middle = (class as f64 + 0.5) * width;
    if middle < PIXEL_FROM {
        // A disk, shrinking with the magnitude to a single pixel.
        let disks = (PIXEL_FROM / width).round().max(1.0) - 1.0;
        let along = if disks > 0.0 {
            class as f64 / disks
        } else {
            1.0
        };
        let radius =
            BRIGHTEST_RADIUS - along * (BRIGHTEST_RADIUS - PIXEL_RADIUS);
        (class, radius, 1.0)
    } else {
        // A single pixel, a fifth of a magnitude's dimming for each
        // magnitude: gentler than the sky's own, so that the faintest still
        // show on a screen.
        let bright = 10f64.powf(-0.2 * (middle - PIXEL_FROM));
        (class, PIXEL_RADIUS, bright as f32)
    }
}

/// The stars in the sky, each somewhere above the ground, in classes by
/// their apparent brightness: the brightest small disks, shrinking to a
/// single pixel, and the fainter single pixels, dimmer the fainter.
///
/// Placed by a little generator seeded from the landscape, so that one
/// landscape has one sky, and the same each time the window is opened.
///
/// # Arguments
///
/// * `f` - The lander's figures.
///
/// # Returns
///
/// Each class's picture commands, and its brightness, from nought to one,
/// the brightest first.
pub fn stars(f: &Resolved) -> Vec<(String, f32)> {
    // xorshift64, from the ground's points.
    let mut state = f
        .ground
        .iter()
        .fold(0x9E37_79B9_7F4A_7C15_u64, |h, &(x, y)| {
            (h ^ x.to_bits()).rotate_left(23) ^ y.to_bits()
        })
        | 1;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let metres = f.width / SCENE_WIDTH;
    let mut classes: Vec<(String, f32)> = (0..STAR_CLASSES)
        .map(|class| {
            let middle = (class as f64 + 0.5) * FAINTEST / STAR_CLASSES as f64;
            (String::new(), star_class(middle).2)
        })
        .collect();
    for _ in 0..STARS {
        let x = next() * f.width;
        let lowest = ground_at(f, x) + STAR_CLEARANCE * f.height;
        let (u, height) = (next(), next());
        if lowest >= f.height {
            continue;
        }
        let y = lowest + height * (f.height - lowest);
        let (class, radius, _) = star_class(magnitude(u));
        let commands = &mut classes[class].0;
        if !commands.is_empty() {
            commands.push(' ');
        }
        commands.push_str(&circle(f, (x, y), radius * metres));
    }
    classes
}

/// The lander's shapes or its single lines, where the place between its
/// feet is.
///
/// A line of more than two points is a shape -- the body, the cabin, the
/// nozzle, a foot -- which is filled; one of two is a strut, a leg or the
/// antenna, which is not. The window draws the two differently: see
/// `ui/lander.slint`.
///
/// # Arguments
///
/// * `f`      - The lander's figures.
/// * `place`  - Where the place between its feet is, in metres.
/// * `shapes` - True for its shapes, false for its single lines.
///
/// # Returns
///
/// The picture's commands.
pub fn lander(f: &Resolved, place: Point, shapes: bool) -> String {
    f.outline
        .iter()
        .filter(|line| (line.len() > 2) == shapes)
        .map(|line| polyline(f, line, place))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A circle, as two arcs.
///
/// # Arguments
///
/// * `f`      - The lander's figures.
/// * `centre` - Its centre, in metres.
/// * `radius` - Its radius, in metres.
///
/// # Returns
///
/// The picture's commands.
fn circle(f: &Resolved, (x, y): Point, radius: f64) -> String {
    format!(
        "M {} A {r:.2} {r:.2} 0 1 0 {} A {r:.2} {r:.2} 0 1 0 {} Z",
        at(f, (x - radius, y)),
        at(f, (x + radius, y)),
        at(f, (x - radius, y)),
        r = radius
    )
}

/// The smiling face drawn over the lander on a landing: a round face, two
/// eyes and a smile, as wide as most of the lander.
///
/// # Arguments
///
/// * `f`     - The lander's figures.
/// * `place` - Where the place between its feet is, in metres.
///
/// # Returns
///
/// The picture's commands.
pub fn face(f: &Resolved, place: Point) -> String {
    let (x0, x1, y0, y1) = f.extent();
    let centre = (place.0 + (x0 + x1) / 2.0, place.1 + (y0 + y1) / 2.0);
    let r = 0.6 * (x1 - x0).min(y1 - y0) / 2.0;
    let eye = r * 0.14;
    let (cx, cy) = centre;
    let smile = (cx - r * 0.5, cy - r * 0.25);
    format!(
        "{} {} {} M {} A {:.2} {:.2} 0 0 0 {}",
        circle(f, centre, r),
        circle(f, (cx - r * 0.35, cy + r * 0.3), eye),
        circle(f, (cx + r * 0.35, cy + r * 0.3), eye),
        at(f, smile),
        r * 0.55,
        r * 0.55,
        at(f, (cx + r * 0.5, cy - r * 0.25)),
    )
}

/// The joystick's axes, and where its dot goes, from where the pointer is
/// held.
///
/// Each axis reads on its own: nought while the pointer is within the
/// square's span along it, and beyond, the distance past the square's edge
/// over a rectangle's reach, held at one. X is positive to the right and Y
/// positive upward. The dot follows the pointer within the rectangle that
/// encloses the plus.
///
/// # Arguments
///
/// * `dx` - How far right of the square's middle the pointer is, in
///          logical pixels.
/// * `dy` - How far below it, in logical pixels.
///
/// # Returns
///
/// The X axis and the Y axis, each within minus one and one, and the dot's
/// place from the square's middle, right and down, in logical pixels.
pub fn axes(dx: f32, dy: f32) -> (f64, f64, f32, f32) {
    let half = SQUARE / 2.0;
    let reach = half + ARM;
    let along = |d: f32| -> f64 {
        let past = d.abs() - half;
        if past <= 0.0 {
            0.0
        } else {
            f64::from((past / ARM).min(1.0)).copysign(f64::from(d))
        }
    };
    (
        along(dx),
        -along(dy),
        dx.clamp(-reach, reach),
        dy.clamp(-reach, reach),
    )
}

/// Holds the joystick at two axes, as a pilot holding it would, and puts its
/// dot where they say.
///
/// # Arguments
///
/// * `ui`    - tcs-sim's window.
/// * `scene` - The world's lander, whose joystick it is.
/// * `stick` - The joystick's place among the world's components.
/// * `axes`  - The X and Y axes, each from minus one to one.
fn hold_stick(ui: &MainWindow, scene: &Scene, stick: usize, axes: (f64, f64)) {
    let now = Instant::now();
    for (terminal, value) in [(SET_X, axes.0), (SET_Y, axes.1)] {
        scene.world.drive_by_hand(
            Place {
                component: stick,
                terminal,
            },
            value,
            now,
        );
    }
    let pane = ui.global::<LanderScene>();
    pane.set_dot_x(dot_of(axes.0));
    pane.set_dot_y(-dot_of(axes.1));
}

/// Makes room in tcs-sim's window for the lander's pane, at the left of its
/// payload panels: the pane measured, as a panel is, and the window told.
/// Before the window is measured itself, so that its chrome counts the pane.
///
/// # Arguments
///
/// * `ui` - tcs-sim's window.
pub fn make_room(ui: &MainWindow) {
    if let Ok(probe) = LanderProbe::new() {
        ui.set_lander_width(probe.get_pane_width());
        ui.set_lander_height(probe.get_pane_height());
    }
}

/// Shows the lander in tcs-sim's window, kept up to the world, its joystick
/// driving the world's.
///
/// # Arguments
///
/// * `ui`    - tcs-sim's window, given room for the pane by [`make_room`].
/// * `scene` - The world's lander.
///
/// # Returns
///
/// The timer that keeps the pane up to the world, to be kept while tcs-sim
/// runs.
pub fn attach(ui: &MainWindow, scene: Scene) -> slint::Timer {
    let pane = ui.global::<LanderScene>();
    draw_landscape(&pane, &scene.figures);

    let scene = Rc::new(scene);
    // The autopilot, while it flies the lander: see `Autopilot`.
    let pilot: Rc<RefCell<Option<Autopilot>>> = Rc::default();
    pane.set_has_joystick(scene.joystick.is_some());
    {
        let held = Rc::clone(&scene);
        let weak = ui.as_weak();
        pane.on_pause_clicked(move || {
            let now = Instant::now();
            if held.world.paused() {
                held.world.resume(now);
            } else {
                held.world.pause(now);
            }
            if let Some(ui) = weak.upgrade() {
                ui.global::<LanderScene>().set_paused(held.world.paused());
            }
        });
    }
    if let Some(stick) = scene.joystick {
        let held = Rc::clone(&scene);
        let flying = Rc::clone(&pilot);
        let weak = ui.as_weak();
        pane.on_autoland_clicked(move || {
            let Some(ui) = weak.upgrade() else { return };
            let mut flying = flying.borrow_mut();
            if flying.take().is_some() {
                // Let go of, as a pilot letting go would.
                hold_stick(&ui, &held, stick, (0.0, 0.0));
            } else {
                *flying = Some(Autopilot::default());
            }
            ui.global::<LanderScene>().set_autoland(flying.is_some());
        });
    }
    if let Some(stick) = scene.joystick {
        let held = Rc::clone(&scene);
        let flying = Rc::clone(&pilot);
        let weak = ui.as_weak();
        pane.on_stick_moved(move |dx, dy| {
            // The pilot takes over from the autopilot.
            if flying.borrow_mut().take().is_some() {
                if let Some(ui) = weak.upgrade() {
                    ui.global::<LanderScene>().set_autoland(false);
                }
            }
            let (x, y, dot_x, dot_y) = axes(dx, dy);
            let now = Instant::now();
            let drive = |terminal, value| {
                held.world.drive_by_hand(
                    Place {
                        component: stick,
                        terminal,
                    },
                    value,
                    now,
                )
            };
            drive(SET_X, x);
            drive(SET_Y, y);
            if let Some(ui) = weak.upgrade() {
                let pane = ui.global::<LanderScene>();
                pane.set_dot_x(dot_x);
                pane.set_dot_y(dot_y);
            }
        });
        let held = Rc::clone(&scene);
        let weak = ui.as_weak();
        pane.on_stick_released(move || {
            let now = Instant::now();
            for terminal in [SET_X, SET_Y] {
                held.world.drive_by_hand(
                    Place {
                        component: stick,
                        terminal,
                    },
                    0.0,
                    now,
                );
            }
            if let Some(ui) = weak.upgrade() {
                let pane = ui.global::<LanderScene>();
                pane.set_dot_x(0.0);
                pane.set_dot_y(0.0);
            }
        });
    }

    let opened = Instant::now();
    show(&pane, &scene, 0.0);
    let timer = slint::Timer::default();
    {
        let weak = ui.as_weak();
        let scene = Rc::clone(&scene);
        timer.start(slint::TimerMode::Repeated, REFRESH, move || {
            let Some(ui) = weak.upgrade() else { return };
            show(
                &ui.global::<LanderScene>(),
                &scene,
                opened.elapsed().as_secs_f64(),
            );
            fly(&ui, &scene, &pilot);
        });
    }
    timer
}

/// Has the autopilot, while it flies the lander, hold the joystick where it
/// wants it; and lets go, and stops it, once the lander has landed or
/// crashed. Nothing while the world is paused.
///
/// # Arguments
///
/// * `ui`    - tcs-sim's window.
/// * `scene` - The world's lander.
/// * `pilot` - The autopilot, while it flies.
fn fly(ui: &MainWindow, scene: &Scene, pilot: &RefCell<Option<Autopilot>>) {
    let Some(stick) = scene.joystick else { return };
    let mut flying = pilot.borrow_mut();
    let Some(autopilot) = flying.as_mut() else {
        return;
    };
    if scene.world.paused() {
        return;
    }
    let out = scene.world.glance_component(scene.lander, Instant::now());
    if out[LANDED] > 0.5 || out[CRASHED] > 0.5 {
        *flying = None;
        hold_stick(ui, scene, stick, (0.0, 0.0));
        ui.global::<LanderScene>().set_autoland(false);
        return;
    }
    let axes = autopilot.steer(&scene.figures, &out);
    hold_stick(ui, scene, stick, axes);
}

/// Draws what does not move: the display's size, the ground, the pad and
/// the stars.
///
/// # Arguments
///
/// * `pane`   - The lander's pane, in tcs-sim's window.
/// * `f`      - The lander's figures.
fn draw_landscape(pane: &LanderScene<'_>, f: &Resolved) {
    pane.set_world_width(f.width as f32);
    pane.set_world_height(f.height as f32);
    pane.set_ground_fill(ground_fill(f).into());
    pane.set_ground_line(ground_line(f).into());
    pane.set_pad(pad(f).into());
    pane.set_pad_lights(pad_lights(f).into());
    let classes: Vec<StarClass> = stars(f)
        .into_iter()
        .map(|(commands, brightness)| StarClass {
            commands: commands.into(),
            brightness,
        })
        .collect();
    pane.set_stars(slint::ModelRc::new(slint::VecModel::from(classes)));
}

/// Brings the window up to the world, and the stars to how bright they are
/// now.
///
/// # Arguments
///
/// * `pane`    - The lander's pane, in tcs-sim's window.
/// * `scene`   - The world's lander.
/// * `seconds` - The time since the window opened, in seconds.
fn show(pane: &LanderScene<'_>, scene: &Scene, seconds: f64) {
    let out = scene.world.glance_component(scene.lander, Instant::now());
    paint(pane, &scene.figures, &out);
    pane.set_pad_lit(pad_lit(seconds));
}

/// Shows the lander as its outputs say it is.
///
/// # Arguments
///
/// * `pane`   - The lander's pane, in tcs-sim's window.
/// * `f`      - The lander's figures.
/// * `out`    - Its terminals' values, in its type's order.
fn paint(pane: &LanderScene<'_>, f: &Resolved, out: &[f64]) {
    let place = (out[X], out[Y]);
    pane.set_lander_lines(lander(f, place, false).into());
    pane.set_lander_shapes(lander(f, place, true).into());
    let landed = out[LANDED] > 0.5;
    pane.set_face(if landed {
        face(f, place).into()
    } else {
        "".into()
    });
    pane.set_state(
        if landed {
            "Landed"
        } else if out[CRASHED] > 0.5 {
            "Crashed"
        } else {
            "Flying"
        }
        .into(),
    );
    pane.set_fuel(out[FUEL] as f32);
    pane.set_position_x(format!("{:.1}", out[X]).into());
    pane.set_position_y(format!("{:.1}", out[Y]).into());
    pane.set_velocity_x(format!("{:.2}", out[VELOCITY_X]).into());
    pane.set_velocity_y(format!("{:.2}", out[VELOCITY_Y]).into());
    pane.set_acceleration_x(format!("{:.3}", out[ACCELERATION_X]).into());
    pane.set_acceleration_y(format!("{:.3}", out[ACCELERATION_Y]).into());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lander example's figure: a body with its corners cut, a cabin on
    /// it with an antenna, a nozzle under it, and two splayed legs, each
    /// with a strut and a foot. Thirty metres wide and as tall, its feet at
    /// nought.
    pub(crate) const OUTLINE: &str = "-9,9 9,9 11,11 11,16 9,18 -9,18 \
        -11,16 -11,11 -9,9 | -6,18 -7,22 -4,26 4,26 7,22 6,18 | 0,26 0,30 \
        | -2,30 2,30 | -2.5,9 -3.5,5.5 3.5,5.5 2.5,9 | -11,13 -13.5,2 | \
        -9,9 -13.5,2 | -15,0 -12,0 -13.5,2 -15,0 | 11,13 13.5,2 | 9,9 \
        13.5,2 | 15,0 12,0 13.5,2 15,0 m";

    /// The window watches the world without starting it: before a payload
    /// asks, what it reads is the world as it stands, and what the joystick
    /// drives is held there, both without the clock starting.
    #[test]
    fn the_window_watches_without_starting_the_world() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("lander-sim.yaml");
        let ground = "0,200 200,300 470,90 530,90 800,250 1000,150 m";
        std::fs::write(
            &path,
            format!(
                "simulated_payloads: []\nworld:\n  physics:\n    \
                 components:\n      - type lander:\n          - \
                 component: lander\n            dry_mass: 6000 kg\n            \
                 fuel: 2000 kg\n            isp: 311 s\n            \
                 thrust_x: 12000 N\n            thrust_y: 24000 N\n            \
                 gravity: 1.62 m/s2\n            x: 60 m\n            \
                 y: 540 m\n            width: 1000 m\n            \
                 height: 600 m\n            ground: \"{ground}\"\n            \
                 pad: \"470,90 530,90 m\"\n            \
                 outline: \"{OUTLINE}\"\n            \
                 landing_speed: 3.05 m/s\n            \
                 landing_drift: 1.22 m/s\n            pause: 3 s\n      \
                 - type joystick:\n          - component: stick\n    \
                 connections:\n      - from: stick.x\n        \
                 to: lander.throttle_x\n"
            ),
        )
        .expect("written");
        let sim_file = SimConfigFile::load(&path).expect("it reads");
        let model = sim_file.physics().expect("it resolves").expect("a model");
        let world = World::new(
            &model,
            crate::node::physics::clock::Clock::starting_when_asked(1.0),
            1,
        );
        let lander = model
            .components
            .iter()
            .position(|c| c.name == "lander")
            .expect("the lander");
        let stick = model
            .components
            .iter()
            .position(|c| c.name == "stick")
            .expect("the joystick");
        let now = Instant::now();
        assert_eq!(world.glance_component(lander, now)[Y], 540.0);
        world.drive_by_hand(
            Place {
                component: stick,
                terminal: SET_X,
            },
            0.5,
            now,
        );
        // The joystick's x, its third terminal, gives out what it was set to.
        assert_eq!(world.glance_component(stick, now)[2], 0.5);
        std::thread::sleep(Duration::from_millis(50));
        let later = Instant::now();
        assert_eq!(
            world.glance_component(lander, later)[Y],
            540.0,
            "the lander fell, so the clock started"
        );
    }

    /// Draws the lander's pane, landed on a pad over a varied ground,
    /// into a picture, to be looked at: run by hand, with the picture's path
    /// in `LANDER_SHOT`.
    #[test]
    #[ignore = "draws a picture to be looked at: LANDER_SHOT=<path>"]
    fn draw_the_window() {
        use slint::platform::software_renderer::{
            MinimalSoftwareWindow, RepaintBufferType,
        };

        /// A platform whose windows are drawn into memory.
        struct Headless(Rc<MinimalSoftwareWindow>);
        impl slint::platform::Platform for Headless {
            fn create_window_adapter(
                &self,
            ) -> Result<
                Rc<dyn slint::platform::WindowAdapter>,
                slint::PlatformError,
            > {
                Ok(self.0.clone())
            }
        }
        let adapter = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        slint::platform::set_platform(Box::new(Headless(adapter.clone())))
            .expect("no other platform");
        let outline =
            tcsprotocol::node::physics::lander::lines_of("outline", OUTLINE)
                .expect("the outline reads");
        // LANDER_ZOOM draws a display a tenth the size, for the lander's
        // figure to be seen.
        let zoom = std::env::var("LANDER_ZOOM").is_ok();
        let scale = if zoom { 0.1 } else { 1.0 };
        let f = Resolved {
            width: 1000.0 * scale,
            height: 600.0 * scale,
            ground: vec![
                (0.0, 210.0),
                (90.0, 300.0),
                (180.0, 170.0),
                (300.0, 240.0),
                (420.0, 80.0),
                (480.0, 80.0),
                (600.0, 190.0),
                (720.0, 120.0),
                (850.0, 330.0),
                (1000.0, 200.0),
            ]
            .into_iter()
            .map(|(x, y)| (x * scale, y * scale))
            .collect(),
            pad: 4,
            outline,
            fuel: 2000.0,
            ..Resolved::default()
        };
        // The pane measured first: this platform has one window to draw
        // into, and the probe made after the window would take it over.
        let (width, height) = {
            let probe = LanderProbe::new().expect("a probe");
            (probe.get_pane_width(), probe.get_pane_height())
        };
        let window = MainWindow::new().expect("a window");
        window.set_lander_width(width);
        window.set_lander_height(height);
        let pane = window.global::<LanderScene>();
        draw_landscape(&pane, &f);
        paint(
            &pane,
            &f,
            &[
                0.0,
                0.0,
                450.0 * scale,
                80.0 * scale,
                0.0,
                0.0,
                0.62,
                if zoom { 0.0 } else { 1.0 },
                0.0,
                0.0,
                0.0,
            ],
        );
        let (_, _, dot_x, dot_y) = axes(70.0, -40.0);
        pane.set_dot_x(dot_x);
        pane.set_dot_y(dot_y);
        window.show().expect("shown");
        window.window().set_size(slint::PhysicalSize::new(
            window.get_lander_width() as u32 + 16,
            window.get_least_height() as u32,
        ));
        let size = window.window().size();
        let picture = window.window().take_snapshot().expect("drawn");
        let path = std::env::var("LANDER_SHOT").expect("LANDER_SHOT");
        let file = std::fs::File::create(&path).expect("the file");
        let mut png = png::Encoder::new(file, size.width, size.height);
        png.set_color(png::ColorType::Rgba);
        png.set_depth(png::BitDepth::Eight);
        png.write_header()
            .and_then(|mut w| w.write_image_data(picture.as_bytes()))
            .expect("written");
    }

    /// A small landscape: flat ground at sixty metres, the pad in the
    /// middle, and a square lander.
    ///
    /// # Returns
    ///
    /// Its figures.
    fn figures() -> Resolved {
        Resolved {
            width: 1000.0,
            height: 600.0,
            ground: vec![
                (0.0, 60.0),
                (470.0, 60.0),
                (530.0, 60.0),
                (1000.0, 60.0),
            ],
            pad: 1,
            outline: vec![vec![(-15.0, 0.0), (15.0, 0.0), (15.0, 30.0)]],
            ..Resolved::default()
        }
    }

    /// The ground is filled down to the bottom of the display, with Y
    /// counted down from the top, and the pad has its two upright marks.
    #[test]
    fn the_landscape_is_drawn_to_the_bottom() {
        let f = figures();
        assert_eq!(
            ground_fill(&f),
            "M 0.00 600.00 L 0.00 540.00 L 470.00 540.00 L 530.00 540.00 \
             L 1000.00 540.00 L 1000.00 600.00 Z"
        );
        assert_eq!(pad(&f).matches('M').count(), 3);
        assert!(pad(&f).contains("M 470.00 540.00 L 470.00 528.00"));
    }

    /// Flies a lander by the autopilot against its own physics, each
    /// command held back a tenth of a second as the vehicles hold it back.
    ///
    /// # Arguments
    ///
    /// * `ground` - The ground's points.
    /// * `pad`    - Where the pad's left end is among them.
    /// * `start`  - Where the lander starts.
    ///
    /// # Returns
    ///
    /// Whether it landed, true, or crashed, false, and when, in simulated
    /// seconds; `None` where it did neither in five minutes.
    fn autoland(
        ground: Vec<Point>,
        pad: usize,
        start: Point,
    ) -> Option<(bool, f64)> {
        use crate::node::physics::component::Component;
        use crate::node::physics::lander::Lander;
        let outline =
            tcsprotocol::node::physics::lander::lines_of("outline", OUTLINE)
                .expect("the outline reads");
        let f = Resolved {
            dry_mass: 5500.0,
            fuel: 500.0,
            isp: Duration::from_secs(311),
            thrust_x: 12000.0,
            thrust_y: 24000.0,
            gravity: 1.62,
            start,
            width: 1000.0,
            height: 600.0,
            ground,
            pad,
            outline,
            landing_speed: 3.05,
            landing_drift: 1.22,
            pause: Duration::from_secs(3),
        };
        let mut lander = Lander::new(f.clone());
        let terminals = tcsprotocol::node::physics::lander::TERMINALS.len();
        let mut pilot = Autopilot::default();
        let step = 0.05;
        // What the vehicles have yet to pass on: two steps' worth.
        let mut on_the_way =
            std::collections::VecDeque::from([(0.0, 0.0), (0.0, 0.0)]);
        let mut now = 0.0;
        while now < 300.0 {
            let out: Vec<f64> =
                (0..terminals).map(|t| lander.output(t)).collect();
            if out[LANDED] > 0.5 {
                return Some((true, now));
            }
            if out[CRASHED] > 0.5 {
                return Some((false, now));
            }
            on_the_way.push_back(pilot.steer(&f, &out));
            let (x, y) = on_the_way.pop_front().expect("two steps held");
            let mut inputs = vec![0.0; terminals];
            inputs[0] = x;
            inputs[1] = y;
            lander.hold(&inputs);
            let to = now + step;
            let mut at = now;
            while let Some(event) =
                lander.next_event(at).filter(|event| *event <= to)
            {
                lander.advance(at, event);
                lander.event(event);
                at = event;
            }
            lander.advance(at, to);
            now = to;
        }
        None
    }

    /// The autopilot lands the lander on the shipped landscape, from the
    /// shipped start, and on one with high ground between the start and a
    /// pad at the far side, within the time the fuel lasts.
    #[test]
    fn the_autopilot_lands() {
        let shipped = vec![
            (0.0, 210.0),
            (90.0, 300.0),
            (180.0, 170.0),
            (300.0, 240.0),
            (420.0, 80.0),
            (480.0, 80.0),
            (600.0, 190.0),
            (720.0, 120.0),
            (850.0, 330.0),
            (1000.0, 200.0),
        ];
        let said = autoland(shipped, 4, (60.0, 540.0));
        assert!(matches!(said, Some((true, _))), "{said:?}");

        let high = vec![
            (0.0, 100.0),
            (200.0, 350.0),
            (400.0, 50.0),
            (600.0, 250.0),
            (700.0, 30.0),
            (760.0, 30.0),
            (900.0, 200.0),
            (1000.0, 150.0),
        ];
        let said = autoland(high, 4, (60.0, 540.0));
        assert!(matches!(said, Some((true, _))), "{said:?}");
    }

    /// Stars are about three times as many at each magnitude as at the one
    /// brighter, the brightest are disks shrinking to a single pixel, and
    /// fainter ones are single pixels, dimmer the fainter.
    #[test]
    fn the_stars_are_as_bright_as_the_sky_has_them() {
        assert_eq!(magnitude(0.0), 0.0);
        assert!((magnitude(1.0) - FAINTEST).abs() < 1e-9);
        // How many of a thousand evenly drawn fall in each magnitude.
        let mut counts = [0usize; 6];
        for n in 0..1000 {
            let m = magnitude((n as f64 + 0.5) / 1000.0);
            counts[(m as usize).min(5)] += 1;
        }
        for pair in counts[2..].windows(2) {
            let ratio = pair[1] as f64 / pair[0] as f64;
            assert!((2.5..3.5).contains(&ratio), "{counts:?}");
        }
        let classes: Vec<(usize, f64, f32)> = (0..STAR_CLASSES)
            .map(|c| star_class((c as f64 + 0.5) * FAINTEST / 8.0))
            .collect();
        assert_eq!(classes[0].1, BRIGHTEST_RADIUS);
        for pair in classes.windows(2) {
            let ((_, r0, b0), (_, r1, b1)) = (pair[0], pair[1]);
            assert!(r1 <= r0 && b1 <= b0, "{classes:?}");
            assert!(r1 < r0 || b1 < b0, "{classes:?}");
        }
        assert_eq!(classes[STAR_CLASSES - 1].1, PIXEL_RADIUS);
        assert!(classes[STAR_CLASSES - 1].2 < 0.5, "{classes:?}");
    }

    /// A light sits on top of each end mark of the pad, and the lights are
    /// lit for half of each flash.
    #[test]
    fn the_pads_lights_are_on_its_end_marks_and_flash() {
        let f = figures();
        let ((x0, y), (x1, _)) = f.pad_ends();
        let lights = pad_lights(&f);
        // Two circles, each begun at its left and closed.
        assert_eq!(lights.matches('M').count(), 2, "{lights}");
        assert_eq!(lights.matches('Z').count(), 2, "{lights}");
        let top = y + PAD_MARK * f.height + PAD_LIGHT;
        for x in [x0, x1] {
            let left = format!("M {}", at(&f, (x - PAD_LIGHT, top)));
            assert!(lights.contains(&left), "{left} in {lights}");
        }
        assert!(pad_lit(0.0) && pad_lit(0.4));
        assert!(!pad_lit(0.5) && !pad_lit(0.9));
        assert!(pad_lit(1.2));
    }

    /// The lander is drawn where it is, its outline moved by its place.
    #[test]
    fn the_lander_is_drawn_where_it_is() {
        assert_eq!(
            lander(&figures(), (500.0, 100.0), true),
            "M 485.00 500.00 L 515.00 500.00 L 515.00 470.00"
        );
        assert_eq!(lander(&figures(), (500.0, 100.0), false), "");
    }

    /// The face is three circles and a smile.
    #[test]
    fn the_face_smiles() {
        let face = face(&figures(), (500.0, 100.0));
        assert_eq!(face.matches(" Z").count(), 3);
        assert_eq!(face.matches(" A ").count(), 7);
    }

    /// The joystick reads nothing inside the square, rises linearly across
    /// a rectangle, holds at one beyond it, reads each axis on its own, and
    /// keeps its dot within the plus's enclosing rectangle.
    #[test]
    fn the_joystick_reads_as_it_should() {
        let half = SQUARE / 2.0;
        assert_eq!(axes(0.0, 0.0), (0.0, 0.0, 0.0, 0.0));
        assert_eq!(axes(half, -half).0, 0.0);
        let (x, y, _, _) = axes(half + ARM / 2.0, 0.0);
        assert!((x - 0.5).abs() < 1e-6 && y == 0.0, "{x} {y}");
        let (x, _, _, _) = axes(-(half + ARM / 4.0), 0.0);
        assert!((x + 0.25).abs() < 1e-6, "{x}");
        // Above the square is positive Y, below it negative.
        assert_eq!(axes(0.0, -(half + ARM)).1, 1.0);
        assert_eq!(axes(0.0, half + ARM).1, -1.0);
        // On a diagonal, both at once.
        let (x, y, _, _) = axes(half + ARM, -(half + ARM));
        assert_eq!((x, y), (1.0, 1.0));
        // Beyond the rectangles, held at one, the dot at the edge.
        let (x, y, dot_x, dot_y) = axes(500.0, 400.0);
        assert_eq!((x, y), (1.0, -1.0));
        assert_eq!((dot_x, dot_y), (half + ARM, half + ARM));
    }
}
