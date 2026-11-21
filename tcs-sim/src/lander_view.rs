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

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use tcsos::sync::Arc;
use tcsos::time::{Duration, Instant};

use slint::ComponentHandle;
use tcsprotocol::node::physics::lander::{Point, Resolved};
use tcsprotocol::node::physics::{Place, Resolution};
use tcsprotocol::sim_config::SimConfigFile;

use crate::node::physics::lander::throttles;
use crate::node::physics::sky::{ground_at, stars};
use crate::node::physics::world::World;
use crate::payload::PayloadConfig;
use crate::{LanderProbe, LanderScene, MainWindow};

/// The lander's terminals' places, in
/// `tcsprotocol::node::physics::lander::TERMINALS`'s order: its throttles,
/// inputs, and its outputs.
pub const THROTTLE_X: usize = 0;
pub const THROTTLE_Y: usize = 1;
pub const X: usize = 2;
pub const Y: usize = 3;
pub const VELOCITY_X: usize = 4;
pub const VELOCITY_Y: usize = 5;
pub const FUEL: usize = 6;
pub const LANDED: usize = 7;
pub const CRASHED: usize = 8;
pub const ACCELERATION_X: usize = 9;
pub const ACCELERATION_Y: usize = 10;

/// The lander's autorestart input's place, among the same: past the
/// outputs a ground station's window draws from, so read only where the
/// world's own terminals are, in tcs-sim's.
const AUTORESTART: usize = 12;

/// The joystick's inputs' places, in
/// `tcsprotocol::node::physics::joystick_3_4::TERMINALS`'s order.
const SET_X: usize = 0;
const SET_Y: usize = 1;

/// The joystick's square's side, which is as thick as each of its
/// rectangles, and how far each rectangle reaches beyond the square, in
/// logical pixels: `ui/lander.slint`'s `Joystick34`.
const SQUARE: f32 = 9.45;
const ARM: f32 = 105.0;

/// What the pane says of a crashed lander.
pub const CRASHED_SAID: &str = "On the Moon, no one can hear you CRASH!";

/// How often the window is brought up to the world.
const REFRESH: Duration = Duration::from_millis(16);

/// How wide the landscape is drawn, in logical pixels: `ui/lander.slint`'s
/// scene, which a star's size in pixels is turned into metres by.
const SCENE_WIDTH: f64 = tcsprotocol::node::physics::lander::SKY_WIDTH;

/// The world's lander, and the joystick it is flown with if there is one.
pub struct Scene {
    /// The world.
    world: Arc<World>,
    /// The lander's place among the world's components.
    lander: usize,
    /// What the model says of it.
    figures: Resolved,
    /// The joystick's place among the world's components.
    joystick_3_4: Option<usize>,
    /// What the window last saw of the lander, and its explosion while it
    /// has crashed: see [`Watch`].
    watch: RefCell<Watch>,
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
    let joystick_3_4 = model
        .components
        .iter()
        .position(|c| matches!(c.what, Resolution::Joystick34));
    Some(Scene {
        world,
        lander,
        figures,
        joystick_3_4,
        watch: RefCell::default(),
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

/// The ground's fill, grey with a little brown, and its line, dark brown,
/// as the colours of a picture's pixel.
const GROUND_FILL: [u8; 3] = [0xaa, 0xa1, 0x97];
const GROUND_LINE: [u8; 3] = [0x3b, 0x31, 0x27];

/// The width of the ground's line, in the scene's pixels.
const GROUND_LINE_WIDTH: f64 = 1.5;

/// The ground, painted once into a picture, as the sky is: everything below
/// its line filled, softly at the edge, and the line along it, over nothing
/// above it. One image costs the window the same each frame where the
/// paths of the fill and the line were drawn again whenever the lander
/// moved.
///
/// # Arguments
///
/// * `f` - The lander's figures.
///
/// # Returns
///
/// The picture, as large as [`sky`]'s.
pub fn ground_picture(
    f: &Resolved,
) -> slint::SharedPixelBuffer<slint::Rgba8Pixel> {
    let wide = (SCENE_WIDTH * SKY_DETAIL).round() as u32;
    let high = (SCENE_WIDTH * SKY_DETAIL * f.height / f.width).round() as u32;
    let mut picture =
        slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(wide, high);
    let pixels = picture.make_mut_slice();
    let (sx, sy) = (f64::from(wide) / f.width, f64::from(high) / f.height);
    // The line's points in the picture's pixels, Y down.
    let line: Vec<Point> = f
        .ground
        .iter()
        .map(|&(x, y)| (x * sx, (f.height - y) * sy))
        .collect();
    let half = GROUND_LINE_WIDTH * SKY_DETAIL / 2.0;
    for px in 0..wide {
        let cx = f64::from(px) + 0.5;
        let edge = (f.height - ground_at(f, cx / sx)) * sy;
        let from = (edge - half - 2.0).max(0.0) as u32;
        for py in 0..high {
            let cy = f64::from(py) + 0.5;
            // How much of the pixel is below the line, softly at the edge.
            let fill = (cy - edge + 0.5).clamp(0.0, 1.0);
            // And how much of it the line covers, by its distance from the
            // nearest stretch of it.
            let stroke = if py >= from && cy <= edge + half + 2.0 {
                let near = line
                    .windows(2)
                    .map(|pair| distance(pair[0], pair[1], (cx, cy)))
                    .fold(f64::MAX, f64::min);
                (half + 0.5 - near).clamp(0.0, 1.0)
            } else {
                0.0
            };
            if fill == 0.0 && stroke == 0.0 {
                continue;
            }
            let mix = |a: u8, b: u8| {
                (f64::from(a) * (1.0 - stroke) + f64::from(b) * stroke) as u8
            };
            let alpha = fill.max(stroke);
            pixels[(py * wide + px) as usize] = slint::Rgba8Pixel::new(
                mix(GROUND_FILL[0], GROUND_LINE[0]),
                mix(GROUND_FILL[1], GROUND_LINE[1]),
                mix(GROUND_FILL[2], GROUND_LINE[2]),
                (alpha * 255.0) as u8,
            );
        }
    }
    picture
}

/// How far a point is from a stretch of line.
///
/// # Arguments
///
/// * `a`     - The stretch's one end.
/// * `b`     - Its other.
/// * `point` - The point.
///
/// # Returns
///
/// The distance, in the units the three are in.
fn distance(a: Point, b: Point, point: Point) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let along = (((point.0 - a.0) * dx + (point.1 - a.1) * dy)
        / (dx * dx + dy * dy))
        .clamp(0.0, 1.0);
    (a.0 + along * dx - point.0).hypot(a.1 + along * dy - point.1)
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

/// The radius of each of the lander's lights, in pixels.
const LANDER_LIGHT: f64 = 1.5;

/// How long the lander's white lights take to flash again, and how long each
/// flash lasts, in seconds: brief, as an aircraft's strobes are.
const STROBE_EVERY: f64 = 1.0;
const STROBE_FOR: f64 = 0.15;

/// The lander's upper stage, the cabin: the shape of its outline that
/// reaches highest.
///
/// # Arguments
///
/// * `f` - The lander's figures.
///
/// # Returns
///
/// Its left, right, bottom and top, in metres from the place between the
/// lander's feet; `None` where the outline has no shape.
fn cabin(f: &Resolved) -> Option<(f64, f64, f64, f64)> {
    let top = |line: &Vec<Point>| {
        line.iter().map(|&(_, y)| y).fold(f64::MIN, f64::max)
    };
    let line = f
        .outline
        .iter()
        .filter(|line| line.len() > 2)
        .max_by(|a, b| top(a).total_cmp(&top(b)))?;
    let fold = |pick: fn(&Point) -> f64, best: fn(f64, f64) -> f64, from| {
        line.iter().map(pick).fold(from, best)
    };
    Some((
        fold(|p| p.0, f64::min, f64::MAX),
        fold(|p| p.0, f64::max, f64::MIN),
        fold(|p| p.1, f64::min, f64::MAX),
        fold(|p| p.1, f64::max, f64::MIN),
    ))
}

/// The lander's lights, where it is. The display shows the lander's right
/// side, its front to the display's right: so the steady green light on the
/// right of its cabin, as an aircraft's right is green, faces the viewer, in
/// the middle of the cabin, and the flashing white lights are on the
/// cabin's top and its rear, the display's left, and on the front of the
/// nozzle, halfway up it.
///
/// # Arguments
///
/// * `f`     - The lander's figures.
/// * `place` - Where the place between its feet is, in metres.
///
/// # Returns
///
/// The green light's picture commands and the white lights', each empty
/// where the outline has no cabin.
pub fn lander_lights(f: &Resolved, place: Point) -> (String, String) {
    let Some((left, right, bottom, top)) = cabin(f) else {
        return (String::new(), String::new());
    };
    let r = LANDER_LIGHT * f.width / SCENE_WIDTH;
    let (x, y) = place;
    let (middle, half) = ((left + right) / 2.0, (bottom + top) / 2.0);
    // The nozzle: the shape straight below the cabin's middle that reaches
    // lowest, not the legs' feet to either side. Its front is its right,
    // the lander facing right.
    let bounds = |line: &Vec<Point>| {
        line.iter().fold(
            (f64::MAX, f64::MIN, f64::MAX, f64::MIN),
            |(l, r, b, t), &(x, y)| (l.min(x), r.max(x), b.min(y), t.max(y)),
        )
    };
    let nozzle = f
        .outline
        .iter()
        .filter(|line| line.len() > 2)
        .map(bounds)
        .filter(|&(l, r, _, _)| (l..=r).contains(&middle))
        .min_by(|a, b| a.2.total_cmp(&b.2));
    let front =
        nozzle.map_or((middle, bottom), |(_, r, b, t)| (r, (b + t) / 2.0));
    let green = circle(f, (x + middle, y + half), r);
    let white = [
        (x + middle, y + top),
        (x + front.0, y + front.1),
        (x + left, y + half),
    ]
    .iter()
    .map(|&at| circle(f, at, r))
    .collect::<Vec<_>>()
    .join(" ");
    (green, white)
}

/// Whether the lander's white lights are flashing at a moment.
///
/// # Arguments
///
/// * `seconds` - The time since the window opened, in seconds.
///
/// # Returns
///
/// True during each brief flash.
pub fn strobe_lit(seconds: f64) -> bool {
    seconds.rem_euclid(STROBE_EVERY) < STROBE_FOR
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
        // Up only: there is no thruster on the top to push it down, which
        // gravity does.
        let throttle_y = (RESPONSE * (want_vy - vy) + f.gravity) / up;
        (throttle_x.clamp(-1.0, 1.0), throttle_y.clamp(0.0, 1.0))
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

/// How many pixels the sky's picture has for each of the scene's: enough
/// that it stays sharp as the window is made larger.
const SKY_DETAIL: f64 = 2.0;

/// The sky, painted once into a picture: drawn as one image, it costs the
/// window the same each frame however many stars there are, where a path
/// for each was drawn again whenever the lander moved.
///
/// # Arguments
///
/// * `f` - The lander's figures.
///
/// # Returns
///
/// The picture, white stars on nothing, as wide as the scene is drawn at
/// first times [`SKY_DETAIL`], and as tall as the display's proportions
/// make that.
pub fn sky(f: &Resolved) -> slint::SharedPixelBuffer<slint::Rgba8Pixel> {
    sky_of(f, stars(f))
}

/// The sky with some stars in it, drawn as [`sky`] draws the lander's own:
/// for a ground station, which draws the stars the vehicle sends down.
///
/// # Arguments
///
/// * `f`     - The lander's figures.
/// * `stars` - Each star: its place, in metres; its radius, in the scene's
///             logical pixels; and its brightness, from nought to one.
///
/// # Returns
///
/// The picture, as wide as the scene in logical pixels times
/// `SKY_DETAIL`.
pub fn sky_of(
    f: &Resolved,
    stars: impl IntoIterator<Item = (Point, f64, f32)>,
) -> slint::SharedPixelBuffer<slint::Rgba8Pixel> {
    let wide = (SCENE_WIDTH * SKY_DETAIL).round() as u32;
    let high = (SCENE_WIDTH * SKY_DETAIL * f.height / f.width).round() as u32;
    let mut picture =
        slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(wide, high);
    let pixels = picture.make_mut_slice();
    for ((x, y), radius, brightness) in stars {
        let cx = x / f.width * f64::from(wide);
        let cy = (1.0 - y / f.height) * f64::from(high);
        let r = radius * SKY_DETAIL;
        let reach = r.ceil() as i64 + 1;
        for py in (cy as i64 - reach)..=(cy as i64 + reach) {
            for px in (cx as i64 - reach)..=(cx as i64 + reach) {
                if px < 0 || py < 0 || px >= wide as i64 || py >= high as i64 {
                    continue;
                }
                // How much of the pixel the disk covers, softly at its edge.
                let d = (px as f64 + 0.5 - cx).hypot(py as f64 + 0.5 - cy);
                let cover = (r + 0.5 - d).clamp(0.0, 1.0);
                let alpha = (cover * f64::from(brightness) * 255.0) as u8;
                let pixel =
                    &mut pixels[(py as u32 * wide + px as u32) as usize];
                if alpha > pixel.a {
                    *pixel = slint::Rgba8Pixel::new(255, 255, 255, alpha);
                }
            }
        }
    }
    picture
}

/// The radius the fireball grows to for each kilogram of fuel left at the
/// crash, in metres: about sixty for a full tank of 500 kg.
const FIRE_PER_KG: f64 = 0.12;

/// How quickly the fireball grows, in seconds: most of the way in this.
const FIRE_GROWS: f64 = 0.4;

/// How fast the debris flies for each (metre a second) squared of the speed
/// the lander hit at, in seconds a metre: 10 m/s from a 20 m/s impact, 22.5
/// from 30, so that most of it lands within the display.
const DEBRIS_PER_SPEED_SQUARED: f64 = 0.025;

/// How many fragments the lander breaks into.
const FRAGMENTS: usize = 24;

/// Half a fragment's width, in pixels: large enough that one stopped half
/// buried in the ground is still plain to see.
const FRAGMENT_SIZE: f64 = 3.0;

/// The slowest and fastest a fragment tumbles, in turns a second, either
/// way round.
const SLOWEST_SPIN: f64 = 0.2;
const FASTEST_SPIN: f64 = 3.0;

/// The step a fragment's flight is followed in, in seconds.
const FRAGMENT_STEP: f64 = 0.01;

/// What the window last saw of the lander, and its explosion while it has
/// crashed.
#[derive(Debug, Default)]
pub struct Watch {
    /// Where it was, how fast it was going and how much fuel it had, the
    /// last time it was seen flying: what it crashed with.
    flying: Option<Flying>,
    /// Whether it had crashed then.
    crashed: bool,
    /// Its explosion, from the crash until it starts again.
    explosion: Option<Explosion>,
}

/// The lander as last seen flying.
#[derive(Debug, Clone, Copy)]
struct Flying {
    /// Where the place between its feet was, in metres.
    place: Point,
    /// Its velocity, in metres a second.
    velocity: Point,
    /// The fuel it had, in kilograms.
    fuel: f64,
}

/// A crashed lander's explosion: a fireball of its fuel and a spray of its
/// debris.
#[derive(Debug)]
pub struct Explosion {
    /// When it began, in simulated seconds.
    began: f64,
    /// The fireball's middle, on the lower part of the lander, in metres.
    centre: Point,
    /// The radius the fireball grows to, in metres.
    fire: f64,
    /// Each fragment of debris.
    fragments: Vec<Fragment>,
}

/// One fragment of a crashed lander.
#[derive(Debug, Clone, Copy)]
struct Fragment {
    /// Where it is, in metres.
    place: Point,
    /// Its velocity, in metres a second.
    velocity: Point,
    /// Which way it is turned, in radians.
    angle: f64,
    /// How fast it tumbles, in radians a second, either way round.
    spin: f64,
    /// The simulated time it is followed to, in seconds from the crash.
    at: f64,
    /// Whether it has hit the ground and stopped there.
    stopped: bool,
}

impl Explosion {
    /// The explosion of a lander that crashed as it was last seen flying.
    ///
    /// The fireball is centred on the lower part of the lander and grows to
    /// a radius proportional to the fuel left. The debris flies from where
    /// the lander met the ground, at a speed proportional to the square of
    /// its speed then, spread over ninety degrees either side of the
    /// perpendicular to the ground there.
    ///
    /// # Arguments
    ///
    /// * `f`      - The lander's figures.
    /// * `flying` - The lander as last seen flying.
    /// * `began`  - When it crashed, in simulated seconds.
    ///
    /// # Returns
    ///
    /// The explosion; `None` where the lander was off the display, which
    /// has no ground to explode on.
    fn of(f: &Resolved, flying: Flying, began: f64) -> Option<Explosion> {
        let (x, y) = flying.place;
        if !(0.0..=f.width).contains(&x) || y > f.height {
            return None;
        }
        let (_, _, bottom, top) = f.extent();
        let centre = (x, y + bottom + 0.25 * (top - bottom));
        // The ground the lander met: the stretch under it, and the
        // perpendicular to it, pointing up out of it.
        let pair = f
            .ground
            .windows(2)
            .find(|pair| x <= pair[1].0)
            .unwrap_or(&f.ground[f.ground.len() - 2..]);
        let (dx, dy) = (pair[1].0 - pair[0].0, pair[1].1 - pair[0].1);
        let length = dx.hypot(dy);
        let normal = (-dy / length, dx / length);
        let contact = (x, ground_at(f, x).max(y.min(ground_at(f, x) + 1.0)));
        // xorshift64, from the moment and the place.
        let mut state = (began.to_bits() ^ x.to_bits().rotate_left(17)) | 1;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        let speed = flying.velocity.0.hypot(flying.velocity.1);
        let fastest = DEBRIS_PER_SPEED_SQUARED * speed * speed;
        let up = normal.1.atan2(normal.0);
        let fragments = (0..FRAGMENTS)
            .map(|_| {
                let angle = up + (next() - 0.5) * std::f64::consts::PI;
                let pace = fastest * (0.3 + 0.7 * next());
                let turns =
                    SLOWEST_SPIN + next() * (FASTEST_SPIN - SLOWEST_SPIN);
                let way = if next() < 0.5 { -1.0 } else { 1.0 };
                Fragment {
                    angle: next() * std::f64::consts::TAU,
                    spin: way * turns * std::f64::consts::TAU,
                    place: (
                        contact.0 + 0.5 * normal.0,
                        contact.1 + 0.5 * normal.1,
                    ),
                    velocity: (pace * angle.cos(), pace * angle.sin()),
                    at: 0.0,
                    stopped: false,
                }
            })
            .collect();
        Some(Explosion {
            began,
            centre,
            fire: FIRE_PER_KG * flying.fuel.max(0.0),
            fragments,
        })
    }

    /// Follows each fragment to a moment: falling under the Moon's gravity
    /// until it first meets the ground, where it stops, half buried, however
    /// fast it was going.
    ///
    /// # Arguments
    ///
    /// * `f` - The lander's figures.
    /// * `t` - The moment, in seconds from the crash.
    fn follow(&mut self, f: &Resolved, t: f64) {
        for fragment in &mut self.fragments {
            while !fragment.stopped && fragment.at < t {
                let step = FRAGMENT_STEP.min(t - fragment.at);
                let (x, y) = fragment.place;
                let (vx, vy) = fragment.velocity;
                let next = (x + vx * step, y + vy * step);
                fragment.velocity.1 -= f.gravity * step;
                fragment.angle += fragment.spin * step;
                fragment.at += step;
                if !(0.0..=f.width).contains(&next.0) {
                    fragment.place = next;
                    fragment.stopped = true;
                } else if next.1 <= ground_at(f, next.0) {
                    fragment.place = (next.0, ground_at(f, next.0));
                    fragment.stopped = true;
                } else {
                    fragment.place = next;
                }
            }
        }
    }

    /// The explosion's pictures at a moment.
    ///
    /// # Arguments
    ///
    /// * `f` - The lander's figures.
    /// * `t` - The moment, in seconds from the crash.
    ///
    /// # Returns
    ///
    /// The fireball's outer cloud, its flame and its core, each as picture
    /// commands; how much of it is left, from one at the crash to nought
    /// at the restart; and the debris's commands.
    fn pictures(&self, f: &Resolved, t: f64) -> ([String; 3], f32, String) {
        let lasts = f.pause.as_secs_f64().max(1.0);
        let flicker = 1.0 + 0.06 * (23.0 * t).sin();
        let radius = self.fire * (1.0 - (-t / FIRE_GROWS).exp()) * flicker;
        let ring = |share: f64| {
            if radius * share > 0.0 {
                circle(f, self.centre, radius * share)
            } else {
                String::new()
            }
        };
        let left = (1.0 - t / lasts).clamp(0.0, 1.0) as f32;
        let size = FRAGMENT_SIZE * f.width / SCENE_WIDTH;
        let debris = self
            .fragments
            .iter()
            .filter(|fragment| (0.0..=f.width).contains(&fragment.place.0))
            .map(|fragment| {
                // A square, turned as far as it has tumbled.
                let (x, y) = fragment.place;
                let (sin, cos) = fragment.angle.sin_cos();
                let corner = |u: f64, v: f64| {
                    at(
                        f,
                        (
                            x + size * (u * cos - v * sin),
                            y + size * (u * sin + v * cos),
                        ),
                    )
                };
                format!(
                    "M {} L {} L {} L {} Z",
                    corner(-1.0, -1.0),
                    corner(1.0, -1.0),
                    corner(1.0, 1.0),
                    corner(-1.0, 1.0),
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        ([ring(1.0), ring(0.75), ring(0.45)], left, debris)
    }
}

/// How long the bottom engine's plume is at full thrust, and a side
/// thruster's, in metres.
const PLUME_LONGEST: f64 = 40.0;
const SIDE_PLUME_LONGEST: f64 = 20.0;

/// How much wider a plume is at its tip than where it leaves its engine:
/// a plume spreads in a vacuum.
const PLUME_SPREADS: f64 = 2.0;

/// How long and how wide a plume's core is, as shares of its glow's.
const CORE_LENGTH: f64 = 0.6;
const CORE_WIDTH: f64 = 0.6;

/// How wide a side thruster is where its plume leaves it, in metres.
const SIDE_NOZZLE: f64 = 2.0;

/// The left, right, bottom and top of each of the lander's shapes, in
/// metres from the place between its feet.
///
/// # Arguments
///
/// * `f` - The lander's figures.
///
/// # Returns
///
/// One for each shape, a line of more than two points, in the outline's
/// order.
fn shape_bounds(f: &Resolved) -> Vec<(f64, f64, f64, f64)> {
    f.outline
        .iter()
        .filter(|line| line.len() > 2)
        .map(|line| {
            line.iter().fold(
                (f64::MAX, f64::MIN, f64::MAX, f64::MIN),
                |(l, r, b, t), &(x, y)| {
                    (l.min(x), r.max(x), b.min(y), t.max(y))
                },
            )
        })
        .collect()
}

/// A plume as a picture: a quadrilateral leaving its engine's mouth and
/// spreading to its tip.
///
/// # Arguments
///
/// * `f`      - The lander's figures.
/// * `mouth`  - The middle of the engine's mouth, in metres.
/// * `toward` - Which way the plume goes, a unit vector.
/// * `width`  - How wide the mouth is, in metres.
/// * `length` - How long the plume is, in metres.
///
/// # Returns
///
/// The picture's commands; empty for a plume of no length.
fn plume(
    f: &Resolved,
    mouth: Point,
    toward: Point,
    width: f64,
    length: f64,
) -> String {
    if length <= 0.0 {
        return String::new();
    }
    let across = (-toward.1, toward.0);
    let tip = (mouth.0 + toward.0 * length, mouth.1 + toward.1 * length);
    let (near, far) = (width / 2.0, width * PLUME_SPREADS / 2.0);
    let corner = |at: Point, half: f64, side: f64| {
        let (x, y) = at;
        self::at(f, (x + across.0 * half * side, y + across.1 * half * side))
    };
    format!(
        "M {} L {} L {} L {} Z",
        corner(mouth, near, -1.0),
        corner(mouth, near, 1.0),
        corner(tip, far, 1.0),
        corner(tip, far, -1.0),
    )
}

/// The plumes of the lander's engines, as its throttles say they are firing:
/// the bottom engine's out of the nozzle's foot, pointing down, and a side
/// thruster's out of the body's side, the left one firing to push it right
/// and the right one to push it left. Each is as long as its thrust is a
/// share of its full thrust: its throttle, the joystick's value as the
/// vehicle passes it on, as the engines burn it. None once the fuel is
/// gone or the lander has crashed; standing on the pad, a plume shows
/// while its engine burns.
///
/// # Arguments
///
/// * `f`   - The lander's figures.
/// * `out` - Its terminals' values, in its type's order, its throttles
///           what they are held at.
///
/// # Returns
///
/// The bottom engine's glow and core, and the side thruster's, each as
/// picture commands, and how hard each is firing, from nought to one.
pub fn plumes(f: &Resolved, out: &[f64]) -> ([String; 4], f32, f32) {
    let none = || {
        (
            [String::new(), String::new(), String::new(), String::new()],
            0.0,
            0.0,
        )
    };
    if out[CRASHED] > 0.5 || out[FUEL] <= 0.0 {
        return none();
    }
    let (across, down) = throttles(out[THROTTLE_X], out[THROTTLE_Y]);
    let place = (out[X], out[Y]);
    let shapes = shape_bounds(f);
    let Some(&(left, right, bottom, top)) =
        shapes.iter().max_by(|a, b| a.3.total_cmp(&b.3))
    else {
        return none();
    };
    let middle = (left + right) / 2.0;
    // The nozzle: the shape below the cabin's middle that reaches lowest;
    // the body: the widest shape.
    let nozzle = shapes
        .iter()
        .filter(|&&(l, r, _, _)| (l..=r).contains(&middle))
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .copied()
        .unwrap_or((left, right, bottom, top));
    let body = shapes
        .iter()
        .max_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0)))
        .copied()
        .unwrap_or(nozzle);
    let main = |share: f64, wide: f64| {
        plume(
            f,
            (place.0 + (nozzle.0 + nozzle.1) / 2.0, place.1 + nozzle.2),
            (0.0, -1.0),
            (nozzle.1 - nozzle.0) * wide,
            PLUME_LONGEST * down * share,
        )
    };
    let side_mouth = if across > 0.0 {
        (place.0 + body.0, place.1 + (body.2 + body.3) / 2.0)
    } else {
        (place.0 + body.1, place.1 + (body.2 + body.3) / 2.0)
    };
    let side = |share: f64, wide: f64| {
        plume(
            f,
            side_mouth,
            (-across.signum(), 0.0),
            SIDE_NOZZLE * wide,
            SIDE_PLUME_LONGEST * across.abs() * share,
        )
    };
    (
        [
            main(1.0, 1.0),
            main(CORE_LENGTH, CORE_WIDTH),
            side(1.0, 1.0),
            side(CORE_LENGTH, CORE_WIDTH),
        ],
        down as f32,
        across.abs() as f32,
    )
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
/// * `(x, y)` - Its centre, in metres.
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
    // Three fifths of the lander, which it grows with.
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
/// The X axis, within minus one and one, and the Y axis, within nought and
/// one -- there is no arm below the square, the lander having no thruster to
/// push it down -- and where the two tracking lines go, from the square's
/// middle, right and down, in logical pixels: the vertical one from the left
/// end of the left arm to the right end of the right one, and the
/// horizontal one from halfway up the square to the top arm's far end.
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
        (-along(dy)).max(0.0),
        dx.clamp(-reach, reach),
        dy.clamp(-reach, 0.0),
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
    // The horizontal line halfway up the square for no thrust, and out
    // along the top arm for more.
    pane.set_dot_y(-dot_of(axes.1.max(0.0)));
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
    // Whether the joystick beckons: at first, and again each time the
    // lander starts again after a crash with nobody flying it -- neither the
    // autopilot nor a hand on the joystick -- until the
    // joystick is taken hold of or a button pressed. See `Joystick34`.
    let beckoning = Rc::new(Cell::new(scene.joystick_3_4.is_some()));
    // Whether the joystick is held now, by the pointer: a lander that starts
    // again meanwhile is being flown, and the joystick does not beckon.
    let holding = Rc::new(Cell::new(false));
    pane.set_has_joystick_3_4(scene.joystick_3_4.is_some());
    {
        let held = Rc::clone(&scene);
        let weak = ui.as_weak();
        let beckon = Rc::clone(&beckoning);
        pane.on_pause_clicked(move || {
            beckon.set(false);
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
    if let Some(stick) = scene.joystick_3_4 {
        let held = Rc::clone(&scene);
        let flying = Rc::clone(&pilot);
        let weak = ui.as_weak();
        let beckon = Rc::clone(&beckoning);
        pane.on_autoland_clicked(move || {
            beckon.set(false);
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
    if let Some(stick) = scene.joystick_3_4 {
        let held = Rc::clone(&scene);
        let flying = Rc::clone(&pilot);
        let weak = ui.as_weak();
        let beckon = Rc::clone(&beckoning);
        let hold = Rc::clone(&holding);
        pane.on_stick_moved(move |dx, dy| {
            // Paused, the joystick takes nothing: the window does not take
            // hold of it either.
            if held.world.paused() {
                return;
            }
            beckon.set(false);
            hold.set(true);
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
        let hold = Rc::clone(&holding);
        pane.on_stick_released(move || {
            hold.set(false);
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
        // How the lander was when last looked at, crashed, landed or
        // flying, and the fuel it had, which only a restart raises.
        let was = Cell::new(Stopped::Flying);
        let had_fuel = Cell::new(f64::INFINITY);
        timer.start(slint::TimerMode::Repeated, REFRESH, move || {
            let Some(ui) = weak.upgrade() else { return };
            let seconds = opened.elapsed().as_secs_f64();
            show(&ui.global::<LanderScene>(), &scene, seconds);
            fly(&ui, &scene, &pilot);
            let out =
                scene.world.glance_component(scene.lander, Instant::now());
            let now = if out[CRASHED] > 0.5 {
                Stopped::Crashed
            } else if out[LANDED] > 0.5 {
                Stopped::Pad
            } else {
                Stopped::Flying
            };
            let before = was.replace(now);
            let refuelled = out[FUEL] > had_fuel.replace(out[FUEL]) + 1e-9;
            let after_crash = before == Stopped::Crashed && now != before;
            if after_crash || refuelled {
                // Started again. With autorestart on, after a crash or from
                // the pad, the autopilot flies the next flight, so that a
                // booth nobody is at flies itself; taking hold of the
                // joystick hands it back.
                let idle = pilot.borrow().is_none()
                    && !holding.get()
                    && scene.joystick_3_4.is_some();
                match on_start(before, idle, out[AUTORESTART] > 0.5) {
                    Started::Flown => {
                        *pilot.borrow_mut() = Some(Autopilot::default());
                        ui.global::<LanderScene>().set_autoland(true);
                    }
                    Started::Beckoning => beckoning.set(true),
                    Started::Alone => {}
                }
            }
            let pane = ui.global::<LanderScene>();
            pane.set_beckon(beckoning.get());
            pane.set_beckon_lit(beckoning.get() && beckon_lit(seconds));
            regions(&ui);
        });
    }
    timer
}

/// How the lander was before it started again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stopped {
    /// Crashed.
    Crashed,
    /// Landed, on the pad.
    Pad,
    /// Flying: started again by a restart in flight.
    Flying,
}

/// What the window does as the lander starts again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Started {
    /// The autopilot flies it: a booth nobody is at flies itself.
    Flown,
    /// The joystick beckons, for someone to fly it.
    Beckoning,
    /// Nothing: someone, or the autopilot, is flying it already.
    Alone,
}

/// What the window does as the lander starts again: with autorestart on,
/// after a crash or from the pad, which is how autorestart starts it, the
/// autopilot flies the next flight; after a crash with autorestart off,
/// restarted by hand, the joystick beckons. Either only while nobody flies
/// it.
///
/// # Arguments
///
/// * `before`      - How it was before it started again.
/// * `idle`        - Whether nobody flies it now, neither the autopilot nor
///                   a hand on the joystick, and there is a joystick.
/// * `autorestart` - Whether the lander's autorestart is on.
///
/// # Returns
///
/// What the window does.
fn on_start(before: Stopped, idle: bool, autorestart: bool) -> Started {
    match (before, idle, autorestart) {
        (Stopped::Crashed | Stopped::Pad, true, true) => Started::Flown,
        (Stopped::Crashed, true, false) => Started::Beckoning,
        _ => Started::Alone,
    }
}

/// How far round the joystick a touch is the joystick's, in logical
/// pixels: as far as a press takes hold of it, `Joystick34`'s `grab`.
const FINGERTIP: f32 = 22.0;

/// How often the beckoning joystick's square flashes, in seconds, and for
/// how much of each it is lit.
const BECKON_EVERY: f64 = 1.0;
const BECKON_FOR: f64 = 0.5;

/// Whether the beckoning joystick's square is lit at a moment.
///
/// # Arguments
///
/// * `seconds` - The time since the window opened, in seconds.
///
/// # Returns
///
/// Whether it is lit.
pub fn beckon_lit(seconds: f64) -> bool {
    seconds.rem_euclid(BECKON_EVERY) < BECKON_FOR
}

/// Tells a web page which part of tcs-sim's window must fit on a small
/// screen -- from the window's top left to the far corner of the lander's
/// pane, its picture and its controls -- and that the joystick keeps the
/// touches that start on it: the panels beside the pane are scrolled to.
/// Nothing on Linux. See `tcs_ui::windows::regions`.
///
/// # Arguments
///
/// * `ui` - tcs-sim's window.
fn regions(ui: &MainWindow) {
    use tcs_ui::windows::Area;

    let pane = ui.global::<LanderScene>();
    tcs_ui::windows::regions(
        ui,
        Area {
            x: 0.0,
            y: 0.0,
            width: pane.get_pane_right(),
            height: pane.get_pane_bottom(),
        },
        // With a fingertip's reach round it, where a press takes hold
        // of the stick as well: see `Joystick34`'s `grab`.
        Area {
            x: pane.get_stick_left() - FINGERTIP,
            y: pane.get_stick_top() - FINGERTIP,
            width: pane.get_stick_width() + 2.0 * FINGERTIP,
            height: pane.get_stick_height() + 2.0 * FINGERTIP,
        },
    );
}

/// Has the autopilot, while it flies the lander, hold the joystick where it
/// wants it; and lets go, and stops it, once the lander has landed, so that
/// whoever is there can fly it off the pad and land again while the lander
/// waits there; with autorestart on, the lander starts again once its pad
/// wait has run, and the autopilot flies that flight: see [`on_start`].
/// While
/// the lander lies crashed it lets go and waits, and flies again from the
/// start once the lander is restarted -- by Restart, or by itself with
/// autorestart on -- so that Autoland pressed during a crash is not lost.
/// Nothing while the world is paused.
///
/// # Arguments
///
/// * `ui`    - tcs-sim's window.
/// * `scene` - The world's lander.
/// * `pilot` - The autopilot, while it flies.
fn fly(ui: &MainWindow, scene: &Scene, pilot: &RefCell<Option<Autopilot>>) {
    let Some(stick) = scene.joystick_3_4 else {
        return;
    };
    let mut flying = pilot.borrow_mut();
    let Some(autopilot) = flying.as_mut() else {
        return;
    };
    if scene.world.paused() {
        return;
    }
    let out = scene.world.glance_component(scene.lander, Instant::now());
    if out[LANDED] > 0.5 {
        hold_stick(ui, scene, stick, (0.0, 0.0));
        *flying = None;
        ui.global::<LanderScene>().set_autoland(false);
        return;
    }
    if out[CRASHED] > 0.5 {
        // Afresh for the next flight, whatever it had worked out of this.
        *autopilot = Autopilot::default();
        hold_stick(ui, scene, stick, (0.0, 0.0));
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
    pane.set_ground(slint::Image::from_rgba8(ground_picture(f)));
    pane.set_pad(pad(f).into());
    pane.set_pad_lights(pad_lights(f).into());
    pane.set_sky(slint::Image::from_rgba8(sky(f)));
}

/// Brings the window up to the world, and the lights to whether they are lit
/// now.
///
/// # Arguments
///
/// * `pane`    - The lander's pane, in tcs-sim's window.
/// * `scene`   - The world's lander.
/// * `seconds` - The time since the window opened, in seconds.
fn show(pane: &LanderScene<'_>, scene: &Scene, seconds: f64) {
    let now = Instant::now();
    let out = scene.world.glance_component(scene.lander, now);
    let frame = scene.watch.borrow_mut().frame(
        &scene.figures,
        &out,
        scene.world.seconds(now),
        seconds,
    );
    set_frame(pane, &frame);
}

/// What the lander's pane shows at a moment that moves: every picture and
/// every word, worked out from the lander's outputs, for a window to be set
/// from. tcs-sim's window sets its pane from the world's lander; a ground
/// station sets its own from what the vehicles send down.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frame {
    /// The lander's single lines, as picture commands; empty while it has
    /// crashed.
    pub lander_lines: String,
    /// Its shapes, filled.
    pub lander_shapes: String,
    /// Its green light.
    pub lander_green: String,
    /// Its white strobes.
    pub lander_strobes: String,
    /// The bottom engine's plume: its glow, its core, and how hard it
    /// fires, from nought to one.
    pub plume_glow: String,
    /// The bottom engine's plume's core.
    pub plume_core: String,
    /// How hard the bottom engine fires.
    pub plume_strength: f32,
    /// The side thruster's plume's glow.
    pub side_plume_glow: String,
    /// The side thruster's plume's core.
    pub side_plume_core: String,
    /// How hard the side thruster fires.
    pub side_plume_strength: f32,
    /// The fireball's outer cloud, after a crash.
    pub fire_outer: String,
    /// The fireball's flame.
    pub fire_flame: String,
    /// The fireball's core.
    pub fire_core: String,
    /// How much of the fireball is left, from one at the crash to nought.
    pub fire_left: f32,
    /// The debris.
    pub debris: String,
    /// The smiling face, on a landing.
    pub face: String,
    /// What the lander is doing, in words.
    pub state: &'static str,
    /// The fuel left, from nought to one.
    pub fuel: f32,
    /// Its position, velocity and acceleration, each axis as the pane says
    /// it.
    pub position_x: String,
    /// Y position.
    pub position_y: String,
    /// X velocity.
    pub velocity_x: String,
    /// Y velocity.
    pub velocity_y: String,
    /// X acceleration.
    pub acceleration_x: String,
    /// Y acceleration.
    pub acceleration_y: String,
    /// Whether the pad's lights are lit.
    pub pad_lit: bool,
    /// Whether the lander's strobes are lit.
    pub strobe_lit: bool,
}

impl Watch {
    /// Works out what the pane shows of the lander, from its outputs: a
    /// crash explodes, from the crash until the lander starts again, the
    /// lander gone meanwhile.
    ///
    /// # Arguments
    ///
    /// * `f`         - The lander's figures.
    /// * `out`       - Its terminals' values, in its type's order:
    ///                 [`THROTTLE_X`] to [`ACCELERATION_Y`].
    /// * `simulated` - The lander's time now, in seconds, which an
    ///                 explosion is followed by.
    /// * `seconds`   - The time since the window opened, in seconds, which
    ///                 the lights flash by.
    ///
    /// # Returns
    ///
    /// The frame.
    pub fn frame(
        &mut self,
        f: &Resolved,
        out: &[f64],
        simulated: f64,
        seconds: f64,
    ) -> Frame {
        let mut frame = painted(f, out);
        frame.pad_lit = pad_lit(seconds);
        frame.strobe_lit = strobe_lit(seconds);
        let crashed = out[CRASHED] > 0.5;
        if !crashed {
            self.flying = Some(Flying {
                place: (out[X], out[Y]),
                velocity: (out[VELOCITY_X], out[VELOCITY_Y]),
                fuel: out[FUEL].clamp(0.0, 1.0) * f.fuel,
            });
            self.explosion = None;
        } else if !self.crashed {
            self.explosion = self
                .flying
                .and_then(|flying| Explosion::of(f, flying, simulated));
        }
        self.crashed = crashed;
        if let Some(explosion) = self.explosion.as_mut() {
            let t = simulated - explosion.began;
            explosion.follow(f, t);
            let ([outer, flame, core], left, debris) = explosion.pictures(f, t);
            frame.fire_outer = outer;
            frame.fire_flame = flame;
            frame.fire_core = core;
            frame.fire_left = left;
            frame.debris = debris;
            frame.lander_lines.clear();
            frame.lander_shapes.clear();
            frame.lander_green.clear();
            frame.lander_strobes.clear();
        }
        frame
    }
}

/// Sets tcs-sim's pane from a frame.
///
/// # Arguments
///
/// * `pane`  - The lander's pane, in tcs-sim's window.
/// * `frame` - What it is to show.
fn set_frame(pane: &LanderScene<'_>, frame: &Frame) {
    pane.set_lander_lines(frame.lander_lines.as_str().into());
    pane.set_lander_shapes(frame.lander_shapes.as_str().into());
    pane.set_lander_green(frame.lander_green.as_str().into());
    pane.set_lander_strobes(frame.lander_strobes.as_str().into());
    pane.set_plume_glow(frame.plume_glow.as_str().into());
    pane.set_plume_core(frame.plume_core.as_str().into());
    pane.set_plume_strength(frame.plume_strength);
    pane.set_side_plume_glow(frame.side_plume_glow.as_str().into());
    pane.set_side_plume_core(frame.side_plume_core.as_str().into());
    pane.set_side_plume_strength(frame.side_plume_strength);
    pane.set_fire_outer(frame.fire_outer.as_str().into());
    pane.set_fire_flame(frame.fire_flame.as_str().into());
    pane.set_fire_core(frame.fire_core.as_str().into());
    pane.set_fire_left(frame.fire_left);
    pane.set_debris(frame.debris.as_str().into());
    pane.set_face(frame.face.as_str().into());
    pane.set_state(frame.state.into());
    pane.set_fuel(frame.fuel);
    pane.set_position_x(frame.position_x.as_str().into());
    pane.set_position_y(frame.position_y.as_str().into());
    pane.set_velocity_x(frame.velocity_x.as_str().into());
    pane.set_velocity_y(frame.velocity_y.as_str().into());
    pane.set_acceleration_x(frame.acceleration_x.as_str().into());
    pane.set_acceleration_y(frame.acceleration_y.as_str().into());
    pane.set_pad_lit(frame.pad_lit);
    pane.set_strobe_lit(frame.strobe_lit);
}

/// The lander as its outputs say it is, before any explosion and with its
/// lights unlit.
///
/// # Arguments
///
/// * `f`   - The lander's figures.
/// * `out` - Its terminals' values, in its type's order.
///
/// # Returns
///
/// The frame.
fn painted(f: &Resolved, out: &[f64]) -> Frame {
    let place = (out[X], out[Y]);
    let (green, white) = lander_lights(f, place);
    let ([glow, core, side_glow, side_core], main, side) = plumes(f, out);
    let landed = out[LANDED] > 0.5;
    Frame {
        lander_lines: lander(f, place, false),
        lander_shapes: lander(f, place, true),
        lander_green: green,
        lander_strobes: white,
        plume_glow: glow,
        plume_core: core,
        plume_strength: main,
        side_plume_glow: side_glow,
        side_plume_core: side_core,
        side_plume_strength: side,
        face: if landed {
            face(f, place)
        } else {
            String::new()
        },
        state: if landed {
            "Landed"
        } else if out[CRASHED] > 0.5 {
            CRASHED_SAID
        } else {
            "Flying"
        },
        fuel: out[FUEL] as f32,
        position_x: format!("{:.1}", out[X]),
        position_y: format!("{:.1}", out[Y]),
        velocity_x: format!("{:.2}", out[VELOCITY_X]),
        velocity_y: format!("{:.2}", out[VELOCITY_Y]),
        acceleration_x: format!("{:.3}", out[ACCELERATION_X]),
        acceleration_y: format!("{:.3}", out[ACCELERATION_Y]),
        ..Frame::default()
    }
}

#[cfg(test)]
pub(crate) mod tests {
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
        tcsos::fs::write(
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
                 - type joystick_3_4:\n          - component: stick\n    \
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
            .expect("the joystick_3_4");
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
        tcsos::thread::sleep(Duration::from_millis(50));
        let later = Instant::now();
        assert_eq!(
            world.glance_component(lander, later)[Y],
            540.0,
            "the lander fell, so the clock started"
        );
    }

    /// The autorestart input's place here is the type's.
    #[test]
    fn the_input_is_the_types() {
        let terminals = tcsprotocol::node::physics::lander::TERMINALS;
        assert_eq!(terminals[AUTORESTART].name, "autorestart");
    }

    /// With autorestart on, a lander starting again after a crash or from
    /// the pad is flown by the autopilot; after a crash with autorestart
    /// off the joystick beckons; and while someone flies it, or after a
    /// restart in flight, nothing changes.
    #[test]
    fn autorestart_is_flown_by_the_autopilot() {
        use Stopped::{Crashed, Flying, Pad};
        assert_eq!(on_start(Crashed, true, true), Started::Flown);
        assert_eq!(on_start(Pad, true, true), Started::Flown);
        assert_eq!(on_start(Crashed, true, false), Started::Beckoning);
        assert_eq!(on_start(Pad, true, false), Started::Alone);
        assert_eq!(on_start(Crashed, false, true), Started::Alone);
        assert_eq!(on_start(Pad, false, true), Started::Alone);
        assert_eq!(on_start(Flying, true, true), Started::Alone);
    }

    /// A paused world's joystick still moves when it is driven by hand, so
    /// that the pilot can set it while the world waits.
    #[test]
    fn a_paused_worlds_joystick_moves() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("lander-sim.yaml");
        let ground = "0,200 200,300 470,90 530,90 800,250 1000,150 m";
        tcsos::fs::write(
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
                 - type joystick_3_4:\n          - component: stick\n    \
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
        let stick = model
            .components
            .iter()
            .position(|c| c.name == "stick")
            .expect("the joystick_3_4");
        let now = Instant::now();
        let _ = world.seconds(now);
        world.pause(now);
        world.drive_by_hand(
            Place {
                component: stick,
                terminal: SET_X,
            },
            0.5,
            Instant::now(),
        );
        assert_eq!(world.glance_component(stick, Instant::now())[2], 0.5);
        let x = Place {
            component: stick,
            terminal: 2,
        };
        assert_eq!(world.read(x, Instant::now()), 0.5);
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
        let zoom = tcsos::env::var("LANDER_ZOOM").is_ok();
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
        set_frame(
            &pane,
            &painted(
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
            ),
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
        let path = tcsos::env::var("LANDER_SHOT").expect("LANDER_SHOT");
        let file = tcsos::fs::File::create(&path).expect("the file");
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
    pub(crate) fn figures() -> Resolved {
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
    /// counted down from the top, its line dark along its top and nothing
    /// above it; and the pad has its two upright marks.
    #[test]
    fn the_landscape_is_drawn_to_the_bottom() {
        let f = figures();
        let picture = ground_picture(&f);
        let (wide, high) = (picture.width(), picture.height());
        let at = |x: u32, y: u32| picture.as_slice()[(y * wide + x) as usize];
        // The ground is sixty metres up a display of 600: a tenth of the
        // picture's height above its bottom.
        let edge = high - high / 10;
        assert_eq!(at(wide / 2, edge - 20).a, 0, "the sky is clear");
        let below = at(wide / 2, (edge + high) / 2);
        assert_eq!(
            (below.r, below.g, below.b, below.a),
            (GROUND_FILL[0], GROUND_FILL[1], GROUND_FILL[2], 255)
        );
        let line = at(wide / 2, edge);
        assert!(line.r < 0x60 && line.a == 255, "{line:?}");
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
            thrust_y: 48000.0,
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
            pad_wait: Duration::from_secs(10),
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

    /// The lander's cabin is its highest shape, the green light faces the
    /// viewer from its middle, the white lights sit on its top, bottom and
    /// rear, and the white ones flash briefly once a second.
    #[test]
    fn the_landers_lights_are_on_its_cabin() {
        let outline =
            tcsprotocol::node::physics::lander::lines_of("outline", OUTLINE)
                .expect("the outline reads");
        let f = Resolved {
            width: 1000.0,
            height: 600.0,
            outline,
            ..figures()
        };
        assert_eq!(cabin(&f), Some((-7.0, 7.0, 18.0, 26.0)));
        let (green, white) = lander_lights(&f, (100.0, 50.0));
        let r = LANDER_LIGHT * f.width / SCENE_WIDTH;
        let left_of = |x: f64, y: f64| format!("M {}", at(&f, (x - r, y)));
        assert!(green.starts_with(&left_of(100.0, 72.0)), "{green}");
        for (x, y) in [(100.0, 76.0), (103.5, 57.25), (93.0, 72.0)] {
            assert!(white.contains(&left_of(x, y)), "{x},{y} in {white}");
        }
        assert!(strobe_lit(0.0) && strobe_lit(1.1));
        assert!(!strobe_lit(0.5) && !strobe_lit(0.2));
    }

    /// A crash's debris flies at a speed in proportion to the square of the
    /// impact speed, spread over ninety degrees either side of the
    /// ground's perpendicular, tumbling at a variety of rates either way,
    /// and stops dead on the ground; its fireball grows with the fuel left.
    #[test]
    fn a_crash_explodes_as_the_impact_says() {
        let f = Resolved {
            gravity: 1.62,
            pause: Duration::from_secs(5),
            pad_wait: Duration::from_secs(10),
            ..figures()
        };
        let flying = |speed: f64, fuel: f64| Flying {
            place: (200.0, 61.0),
            velocity: (0.0, -speed),
            fuel,
        };
        let slow = Explosion::of(&f, flying(10.0, 0.0), 1.0).expect("one");
        let fast = Explosion::of(&f, flying(20.0, 400.0), 1.0).expect("one");
        let fastest = |e: &Explosion| {
            e.fragments
                .iter()
                .map(|d| d.velocity.0.hypot(d.velocity.1))
                .fold(0.0, f64::max)
        };
        // Twice the impact speed, four times the debris's.
        let ratio = fastest(&fast) / fastest(&slow);
        assert!((3.0..5.0).contains(&ratio), "{ratio}");
        // The ground here is level: the perpendicular is straight up, and
        // nothing flies into the ground.
        for d in &fast.fragments {
            assert!(d.velocity.1 >= -1e-9, "{:?}", d.velocity);
        }
        let spins: Vec<f64> = fast.fragments.iter().map(|d| d.spin).collect();
        assert!(
            spins.iter().any(|s| *s > 0.0) && spins.iter().any(|s| *s < 0.0)
        );
        let (least, most) = spins
            .iter()
            .map(|s| s.abs())
            .fold((f64::MAX, 0.0_f64), |(a, b), s| (a.min(s), b.max(s)));
        assert!(most > 3.0 * least, "{least} to {most}");
        assert_eq!(slow.fire, 0.0, "no fuel, no fireball");
        assert!((fast.fire - FIRE_PER_KG * 400.0).abs() < 1e-9);

        // Long enough for every fragment to come down: each stopped on
        // the ground, and turning no more.
        let mut e = fast;
        e.follow(&f, 60.0);
        for d in &e.fragments {
            assert!(d.stopped, "{d:?}");
            if (0.0..=f.width).contains(&d.place.0) {
                assert!((d.place.1 - ground_at(&f, d.place.0)).abs() < 1e-9);
            }
        }
        let turned: Vec<f64> = e.fragments.iter().map(|d| d.angle).collect();
        e.follow(&f, 70.0);
        let still: Vec<f64> = e.fragments.iter().map(|d| d.angle).collect();
        assert_eq!(turned, still);
    }

    /// A plume is as long as its engine's thrust is a share of its full
    /// thrust, as its throttle says, the bottom engine no less than its
    /// floor once lit; there is none for no thrust, an empty tank or a
    /// crash, but there is on the pad; and a push right fires from the
    /// body's left, a push left from its right.
    #[test]
    fn the_plumes_follow_the_thrust() {
        use crate::node::physics::lander::LEAST_THROTTLE;
        let outline =
            tcsprotocol::node::physics::lander::lines_of("outline", OUTLINE)
                .expect("the outline reads");
        let f = Resolved {
            dry_mass: 5500.0,
            fuel: 500.0,
            thrust_x: 12000.0,
            thrust_y: 48000.0,
            gravity: 1.62,
            outline,
            ..figures()
        };
        let mut out = vec![0.0; 11];
        out[X] = 500.0;
        out[Y] = 300.0;
        out[FUEL] = 1.0;
        // Full thrust up, none across.
        out[THROTTLE_Y] = 1.0;
        let ([glow, core, side, _], main, across) = plumes(&f, &out);
        assert!(!glow.is_empty() && !core.is_empty());
        assert!((main - 1.0).abs() < 1e-6 && across == 0.0);
        assert!(side.is_empty(), "{side}");
        // The tip is 40 m below the nozzle's foot, 5.5 m up the lander.
        assert!(
            glow.contains(&format!("{:.2}", f.height - (300.0 + 5.5 - 40.0))),
            "{glow}"
        );

        // Falling with no thrust: no plume.
        out[THROTTLE_Y] = 0.0;
        let ([glow, ..], main, _) = plumes(&f, &out);
        assert!(glow.is_empty() && main == 0.0);

        // A touch up burns at the bottom engine's floor.
        out[THROTTLE_Y] = 0.01;
        let (_, main, _) = plumes(&f, &out);
        assert!((f64::from(main) - LEAST_THROTTLE).abs() < 1e-6, "{main}");
        out[THROTTLE_Y] = 0.0;

        // Half a push right: the left thruster, its plume blowing left.
        out[THROTTLE_X] = 0.5;
        let ([_, _, side, _], _, across) = plumes(&f, &out);
        assert!((across - 0.5).abs() < 1e-6);
        let left_edge = format!("{:.2}", 500.0 - 11.0 - 10.0);
        assert!(side.contains(&left_edge), "{side}");

        // Standing on the pad, the engines still show as they burn.
        out[LANDED] = 1.0;
        out[THROTTLE_Y] = 0.5;
        let ([glow, ..], main, _) = plumes(&f, &out);
        assert!(!glow.is_empty() && (main - 0.5).abs() < 1e-6);

        // The tank dry, or crashed: nothing, whatever the throttles say.
        out[LANDED] = 0.0;
        out[FUEL] = 0.0;
        let ([glow, _, side, _], _, _) = plumes(&f, &out);
        assert!(glow.is_empty() && side.is_empty());
        out[FUEL] = 1.0;
        out[CRASHED] = 1.0;
        let ([glow, _, side, _], _, _) = plumes(&f, &out);
        assert!(glow.is_empty() && side.is_empty());
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
        // Above the square is positive Y; below it, where there is no arm,
        // nought: there is no thrust down.
        assert_eq!(axes(0.0, -(half + ARM)).1, 1.0);
        assert_eq!(axes(0.0, half + ARM).1, 0.0);
        // On a diagonal, both at once.
        let (x, y, _, _) = axes(half + ARM, -(half + ARM));
        assert_eq!((x, y), (1.0, 1.0));
        // Beyond the rectangles, held at one, the lines at the ends: the
        // horizontal one no lower than halfway up the square.
        let (x, y, dot_x, dot_y) = axes(500.0, 400.0);
        assert_eq!((x, y), (1.0, 0.0));
        assert_eq!((dot_x, dot_y), (half + ARM, 0.0));
        let (_, y, _, dot_y) = axes(0.0, -500.0);
        assert_eq!((y, dot_y), (1.0, -(half + ARM)));
    }
}
