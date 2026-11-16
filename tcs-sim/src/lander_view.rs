//! The lander's window: what tcs-sim shows of a world that has a lander.
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
//!
//! Closing the window ends tcs-sim, as closing its payload panels' does.

use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use slint::ComponentHandle;
use tcsprotocol::node::physics::lander::{Point, Resolved};
use tcsprotocol::node::physics::{Place, Resolution};
use tcsprotocol::sim_config::SimConfigFile;

use crate::node::physics::world::World;
use crate::payload::PayloadConfig;
use crate::LanderWindow;

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
const STARS: usize = 140;

/// How many groups the stars twinkle in, each brightening and dimming at a
/// pace of its own: as many as `ui/lander.slint` draws.
const TWINKLE_GROUPS: usize = 4;

/// How long each group takes to brighten and dim again, in seconds.
const TWINKLE_PERIODS: [f64; TWINKLE_GROUPS] = [1.3, 1.9, 2.6, 3.4];

/// How far above the ground a star is at least, as a fraction of the
/// display's height.
const STAR_CLEARANCE: f64 = 0.02;

/// How far a star reaches from its middle, in metres.
const STAR_SIZE: f64 = 1.2;

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
    let mark = f.height / 50.0;
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

/// The stars in the sky, in their twinkling groups: each a small diamond
/// somewhere above the ground.
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
/// Each group's picture commands.
pub fn stars(f: &Resolved) -> [String; TWINKLE_GROUPS] {
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
    let mut groups: [String; TWINKLE_GROUPS] = Default::default();
    for n in 0..STARS {
        let x = next() * f.width;
        let lowest = ground_at(f, x) + STAR_CLEARANCE * f.height;
        if lowest >= f.height {
            continue;
        }
        let y = lowest + next() * (f.height - lowest);
        let s = STAR_SIZE;
        let group = &mut groups[n % TWINKLE_GROUPS];
        if !group.is_empty() {
            group.push(' ');
        }
        group.push_str(&format!(
            "M {} L {} L {} L {} Z",
            at(f, (x - s, y)),
            at(f, (x, y + s)),
            at(f, (x + s, y)),
            at(f, (x, y - s)),
        ));
    }
    groups
}

/// How bright each group of stars is at a moment.
///
/// # Arguments
///
/// * `seconds` - The time since the window opened, in seconds.
///
/// # Returns
///
/// Each group's opacity, from a fifth to one.
pub fn twinkle(seconds: f64) -> [f32; TWINKLE_GROUPS] {
    let mut each = [0.0; TWINKLE_GROUPS];
    for (n, period) in TWINKLE_PERIODS.iter().enumerate() {
        let turn = std::f64::consts::TAU * seconds / period + n as f64;
        each[n] = (0.6 + 0.4 * turn.sin()) as f32;
    }
    each
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

/// Opens the lander's window, kept up to the world, its joystick driving the
/// world's.
///
/// # Arguments
///
/// * `scene` - The world's lander.
/// * `quit`  - What tcs-sim's Quit button does, done when the window is
///             closed.
///
/// # Returns
///
/// The window and the timer that keeps it up to the world, both to be kept
/// while tcs-sim runs; `None` if the window cannot be made.
pub fn open(
    scene: Scene,
    quit: Rc<dyn Fn()>,
) -> Option<(LanderWindow, slint::Timer)> {
    let window = LanderWindow::new().ok()?;
    draw_landscape(&window, &scene.figures);

    let scene = Rc::new(scene);
    if let Some(stick) = scene.joystick {
        let held = Rc::clone(&scene);
        let weak = window.as_weak();
        window.on_stick_moved(move |dx, dy| {
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
            if let Some(window) = weak.upgrade() {
                window.set_dot_x(dot_x);
                window.set_dot_y(dot_y);
            }
        });
        let held = Rc::clone(&scene);
        let weak = window.as_weak();
        window.on_stick_released(move || {
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
            if let Some(window) = weak.upgrade() {
                window.set_dot_x(0.0);
                window.set_dot_y(0.0);
            }
        });
    }

    let opened = Instant::now();
    show(&window, &scene, 0.0);
    let timer = slint::Timer::default();
    {
        let weak = window.as_weak();
        let scene = Rc::clone(&scene);
        timer.start(slint::TimerMode::Repeated, REFRESH, move || {
            if let Some(window) = weak.upgrade() {
                show(&window, &scene, opened.elapsed().as_secs_f64());
            }
        });
    }
    // Closing it ends tcs-sim, its payload panels with it: the lander is
    // flown here, and a run with no way left to fly it is over.
    window.window().on_close_requested(move || {
        quit();
        slint::CloseRequestResponse::HideWindow
    });
    window.show().ok()?;
    Some((window, timer))
}

/// Draws what does not move: the display's size, the ground, the pad and
/// the stars.
///
/// # Arguments
///
/// * `window` - The lander's window.
/// * `f`      - The lander's figures.
fn draw_landscape(window: &LanderWindow, f: &Resolved) {
    window.set_world_width(f.width as f32);
    window.set_world_height(f.height as f32);
    window.set_ground_fill(ground_fill(f).into());
    window.set_ground_line(ground_line(f).into());
    window.set_pad(pad(f).into());
    let [s0, s1, s2, s3] = stars(f);
    window.set_stars_0(s0.into());
    window.set_stars_1(s1.into());
    window.set_stars_2(s2.into());
    window.set_stars_3(s3.into());
}

/// Brings the window up to the world, and the stars to how bright they are
/// now.
///
/// # Arguments
///
/// * `window`  - The lander's window.
/// * `scene`   - The world's lander.
/// * `seconds` - The time since the window opened, in seconds.
fn show(window: &LanderWindow, scene: &Scene, seconds: f64) {
    let out = scene.world.glance_component(scene.lander, Instant::now());
    paint(window, &scene.figures, &out);
    let [t0, t1, t2, t3] = twinkle(seconds);
    window.set_twinkle_0(t0);
    window.set_twinkle_1(t1);
    window.set_twinkle_2(t2);
    window.set_twinkle_3(t3);
}

/// Shows the lander as its outputs say it is.
///
/// # Arguments
///
/// * `window` - The lander's window.
/// * `f`      - The lander's figures.
/// * `out`    - Its terminals' values, in its type's order.
fn paint(window: &LanderWindow, f: &Resolved, out: &[f64]) {
    let place = (out[X], out[Y]);
    window.set_lander_lines(lander(f, place, false).into());
    window.set_lander_shapes(lander(f, place, true).into());
    let landed = out[LANDED] > 0.5;
    window.set_face(if landed {
        face(f, place).into()
    } else {
        "".into()
    });
    window.set_state(
        if landed {
            "Landed"
        } else if out[CRASHED] > 0.5 {
            "Crashed"
        } else {
            "Flying"
        }
        .into(),
    );
    window.set_fuel(out[FUEL] as f32);
    window.set_position_x(format!("{:.1}", out[X]).into());
    window.set_position_y(format!("{:.1}", out[Y]).into());
    window.set_velocity_x(format!("{:.2}", out[VELOCITY_X]).into());
    window.set_velocity_y(format!("{:.2}", out[VELOCITY_Y]).into());
    window.set_acceleration_x(format!("{:.3}", out[ACCELERATION_X]).into());
    window.set_acceleration_y(format!("{:.3}", out[ACCELERATION_Y]).into());
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

    /// Draws the lander's window, landed on a pad over a varied ground,
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
        let window = LanderWindow::new().expect("a window");
        draw_landscape(&window, &f);
        let [t0, t1, t2, t3] = twinkle(0.7);
        window.set_twinkle_0(t0);
        window.set_twinkle_1(t1);
        window.set_twinkle_2(t2);
        window.set_twinkle_3(t3);
        paint(
            &window,
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
        window.set_dot_x(dot_x);
        window.set_dot_y(dot_y);
        window.show().expect("shown");
        window.window().set_size(slint::PhysicalSize::new(816, 900));
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
