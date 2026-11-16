//! A lander: a craft flown in two dimensions over a landscape, under
//! gravity, by thrust along each axis, burning the fuel it carries, until
//! it touches the ground.
//!
//! It takes `throttle_x` and `throttle_y`, each a ratio from minus one to
//! one, which simulated payloads the vehicle drives set: positive pushes it
//! towards positive X, to the right, or positive Y, up, and negative the
//! other way. It gives out where it is, `x` and `y`, in metres, the place
//! between its feet; how fast it is going, `velocity_x` and `velocity_y`, in
//! metres a second; the `fuel` it has left, as a fraction of a full tank;
//! and two decisions, `landed` and `crashed`, each asserted from the moment
//! it touches down until it starts again.
//!
//! # The arithmetic
//!
//! Each axis's thrust is its throttle times its full thrust, and the fuel
//! flows at
//!
//! ```text
//! flow = (|Fx| + |Fy|) / (isp * 9.80665)
//! ```
//!
//! so that its mass, the dry mass and the fuel left, falls as it flies, and
//! the same push accelerates it more as the tank empties. With the throttles
//! held, each axis is the rocket equation of [`super::body`], with gravity
//! on Y alone. When the tank is empty there is no thrust.
//!
//! # The landscape
//!
//! The display is the world: `width` by `height`, from nought at the lower
//! left. The `ground` is a line of points from the left edge to the right,
//! each further right than the last and no steeper between them than 70
//! degrees, none below nought nor higher than 60% of the height. Two of its
//! points, one after the other and level, are the `pad`: twice as wide as
//! the lander, its middle within the middle half of the width, and no higher
//! than a fifth of the height.
//!
//! # Touchdown
//!
//! The lander's `outline`, a set of lines about the place between its feet,
//! is what meets the ground. The moment any of it first meets the ground, the
//! lander is down: landed, if all of it is above the pad, it meets nothing
//! but the pad, and it is going no faster than `landing_speed` down and
//! `landing_drift` sideways; crashed, otherwise. It has crashed as well if it
//! leaves the display altogether. Down, it stays where it is for `pause`, and
//! then starts again where it started, with a full tank. The arithmetic is
//! tcs-sim's `physics::lander`.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::setting::{positive, quantity, time_of};
use super::terminal::{Kind, Terminal};
use crate::unit::Unit;

/// Its terminals.
pub const TERMINALS: &[Terminal] = &[
    Terminal::input("throttle_x", Kind::Value(Unit::Ratio)),
    Terminal::input("throttle_y", Kind::Value(Unit::Ratio)),
    Terminal::output("x", Kind::Value(Unit::Metre)),
    Terminal::output("y", Kind::Value(Unit::Metre)),
    Terminal::output("velocity_x", Kind::Value(Unit::MetrePerSecond)),
    Terminal::output("velocity_y", Kind::Value(Unit::MetrePerSecond)),
    Terminal::output("fuel", Kind::Value(Unit::Ratio)),
    Terminal::output("landed", Kind::Decision),
    Terminal::output("crashed", Kind::Decision),
];

/// Everything it states, by the name a file writes, the
/// `component` naming it apart: the one list, from which an editor builds
/// its form.
pub const SETTINGS: &[&str] = &[
    "dry_mass",
    "fuel",
    "isp",
    "thrust_x",
    "thrust_y",
    "gravity",
    "x",
    "y",
    "width",
    "height",
    "ground",
    "pad",
    "outline",
    "landing_speed",
    "landing_drift",
    "pause",
];

/// The steepest a line of the ground may be, in degrees either way.
pub const STEEPEST: f64 = 70.0;

/// The highest the ground may be, as a fraction of the display's height.
pub const HIGHEST_GROUND: f64 = 0.6;

/// The highest the pad may be, as a fraction of the display's height.
pub const HIGHEST_PAD: f64 = 0.2;

/// How much wider than the lander the pad is.
pub const PAD_TO_LANDER: f64 = 2.0;

/// How near two figures that should be equal have to be, as a fraction of
/// the larger: what a figure written in a file to a few places is good to.
const EQUAL_WITHIN: f64 = 1e-6;

/// A lander, as a physics model states it.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Lander {
    /// What it is called, which is how a connection or a simulated payload
    /// names it.
    #[serde(rename = "component")]
    pub name: String,
    /// Its mass with an empty tank: `"6000 kg"`.
    pub dry_mass: String,
    /// The fuel it starts with: `"2000 kg"`.
    pub fuel: String,
    /// Its thrusters' specific impulse, as a length of time: `"311 s"`.
    pub isp: String,
    /// Its full thrust along X, either way: `"12000 N"`.
    pub thrust_x: String,
    /// Its full thrust along Y, either way: `"24000 N"`.
    pub thrust_y: String,
    /// The gravity it falls in, towards negative Y: `"1.62 m/s2"`, the
    /// Moon's.
    pub gravity: String,
    /// Where it starts, the place between its feet: `"60 m"`.
    pub x: String,
    /// How high it starts: `"540 m"`.
    pub y: String,
    /// The display's width: `"1000 m"`.
    pub width: String,
    /// The display's height: `"600 m"`.
    pub height: String,
    /// The ground, a line of points from the left edge to the right, each
    /// `x,y`, the last followed by its unit: `"0,200 70,260 ... 1000,150
    /// m"`.
    pub ground: String,
    /// The pad, two points of the ground one after the other and level:
    /// `"470,90 530,90 m"`.
    pub pad: String,
    /// The lander's outline: lines of points about the place between its
    /// feet, each `x,y`, the lines parted by `|`, the last point followed by
    /// its unit: `"-15,0 -11,0 | ... m"`.
    pub outline: String,
    /// The fastest it may be going down as it touches the pad and still be
    /// landed: `"3.05 m/s"`, the Apollo lunar module's.
    pub landing_speed: String,
    /// The fastest it may be going sideways as it touches the pad and still
    /// be landed: `"1.22 m/s"`, the Apollo lunar module's.
    pub landing_drift: String,
    /// How long it stays down before it starts again: `"3 s"`.
    pub pause: String,
}

/// A point, in metres: X to the right and Y up.
pub type Point = (f64, f64);

/// A lander with its figures read and its landscape checked.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Resolved {
    /// Kilograms.
    pub dry_mass: f64,
    /// Kilograms.
    pub fuel: f64,
    pub isp: Duration,
    /// Newtons.
    pub thrust_x: f64,
    /// Newtons.
    pub thrust_y: f64,
    /// Metres a second a second, towards negative Y.
    pub gravity: f64,
    /// Where it starts, in metres.
    pub start: Point,
    /// Metres.
    pub width: f64,
    /// Metres.
    pub height: f64,
    /// The ground's points, left to right.
    pub ground: Vec<Point>,
    /// The place in `ground` of the pad's left end; its right end is the
    /// next point.
    pub pad: usize,
    /// The outline's lines, each its points about the place between its
    /// feet.
    pub outline: Vec<Vec<Point>>,
    /// Metres a second.
    pub landing_speed: f64,
    /// Metres a second.
    pub landing_drift: f64,
    pub pause: Duration,
}

impl Resolved {
    /// The lander's extent about the place between its feet: the least and
    /// the most of its outline's X and of its Y.
    ///
    /// # Returns
    ///
    /// `(least x, most x, least y, most y)`, in metres.
    pub fn extent(&self) -> (f64, f64, f64, f64) {
        let points = self.outline.iter().flatten();
        points.fold(
            (f64::MAX, f64::MIN, f64::MAX, f64::MIN),
            |(x0, x1, y0, y1), &(x, y)| {
                (x0.min(x), x1.max(x), y0.min(y), y1.max(y))
            },
        )
    }

    /// The pad's two ends.
    ///
    /// # Returns
    ///
    /// Its left end and its right end.
    pub fn pad_ends(&self) -> (Point, Point) {
        (self.ground[self.pad], self.ground[self.pad + 1])
    }
}

/// Reads a line of points, `x,y x,y ... m`, in metres.
///
/// # Arguments
///
/// * `what` - The setting's name, for the complaint.
/// * `said` - The setting as the file states it.
///
/// # Returns
///
/// The lines of points, parted where the setting writes `|`.
///
/// # Errors
///
/// What is wrong: no unit, a unit other than metres, a point that is not
/// two numbers parted by a comma, or a line with fewer than two points.
pub fn lines_of(what: &str, said: &str) -> Result<Vec<Vec<Point>>, String> {
    let Some(points) = said.trim().strip_suffix(" m") else {
        return Err(format!(
            "{what}: \"{said}\" ends without its unit: write the points, \
             each x,y, and then m, for metres"
        ));
    };
    let mut lines = Vec::new();
    for line in points.split('|') {
        let mut read = Vec::new();
        for point in line.split_whitespace() {
            let numbers: Vec<&str> = point.split(',').collect();
            let parsed = match numbers.as_slice() {
                [x, y] => x.parse::<f64>().ok().zip(y.parse::<f64>().ok()),
                _ => None,
            };
            match parsed {
                Some((x, y)) if x.is_finite() && y.is_finite() => {
                    read.push((x, y))
                }
                _ => {
                    return Err(format!(
                        "{what}: \"{point}\" is not a point: write it x,y, \
                         two numbers and a comma between them"
                    ))
                }
            }
        }
        if read.len() < 2 {
            return Err(format!(
                "{what}: a line has fewer than two points, and a line needs \
                 a point at each end"
            ));
        }
        lines.push(read);
    }
    Ok(lines)
}

/// Says whether two figures are equal, to what a figure written in a file
/// is good to.
///
/// # Arguments
///
/// * `a` - One.
/// * `b` - The other.
///
/// # Returns
///
/// Whether they are.
fn equal(a: f64, b: f64) -> bool {
    (a - b).abs() <= EQUAL_WITHIN * a.abs().max(b.abs()).max(1.0)
}

impl Lander {
    /// Reads its figures, and holds its landscape to the rules the module
    /// says it keeps.
    ///
    /// # Returns
    ///
    /// Them.
    ///
    /// # Errors
    ///
    /// What is wrong with the first that does not read, or with the
    /// landscape, naming it.
    pub fn resolve(&self) -> Result<Resolved, String> {
        let width = positive("width", &self.width, Some("m"))?;
        let height = positive("height", &self.height, Some("m"))?;
        let ground = match lines_of("ground", &self.ground)?.as_slice() {
            [line] => line.clone(),
            _ => return Err("ground: is one line, with no | in it".to_string()),
        };
        let outline = lines_of("outline", &self.outline)?;
        let resolved = Resolved {
            dry_mass: positive("dry_mass", &self.dry_mass, Some("kg"))?,
            fuel: positive("fuel", &self.fuel, Some("kg"))?,
            isp: time_of("isp", &self.isp)?,
            thrust_x: positive("thrust_x", &self.thrust_x, Some("N"))?,
            thrust_y: positive("thrust_y", &self.thrust_y, Some("N"))?,
            gravity: positive("gravity", &self.gravity, Some("m/s2"))?,
            start: (
                quantity("x", &self.x, Some("m"))?,
                quantity("y", &self.y, Some("m"))?,
            ),
            width,
            height,
            pad: pad_of(&ground, &self.pad)?,
            ground,
            outline,
            landing_speed: positive(
                "landing_speed",
                &self.landing_speed,
                Some("m/s"),
            )?,
            landing_drift: positive(
                "landing_drift",
                &self.landing_drift,
                Some("m/s"),
            )?,
            pause: time_of("pause", &self.pause)?,
        };
        landscape_kept(&resolved)?;
        Ok(resolved)
    }
}

/// Finds the pad among the ground's points.
///
/// # Arguments
///
/// * `ground` - The ground's points.
/// * `pad`    - The pad as the file states it.
///
/// # Returns
///
/// The place in `ground` of its left end.
///
/// # Errors
///
/// That it is not two points, or not two points of the ground one after the
/// other.
fn pad_of(ground: &[Point], pad: &str) -> Result<usize, String> {
    let ends = match lines_of("pad", pad)?.as_slice() {
        [line] if line.len() == 2 => (line[0], line[1]),
        _ => return Err("pad: is two points, its ends".to_string()),
    };
    let same = |a: Point, b: Point| equal(a.0, b.0) && equal(a.1, b.1);
    ground
        .windows(2)
        .position(|pair| same(pair[0], ends.0) && same(pair[1], ends.1))
        .ok_or_else(|| {
            "pad: its ends are not two points of the ground, one after the \
             other"
                .to_string()
        })
}

/// Holds a lander's landscape, and where it starts, to the rules the module
/// says it keeps.
///
/// # Arguments
///
/// * `l` - The lander.
///
/// # Errors
///
/// The first rule broken, saying where.
fn landscape_kept(l: &Resolved) -> Result<(), String> {
    let first = l.ground[0];
    let last = l.ground[l.ground.len() - 1];
    if !equal(first.0, 0.0) || !equal(last.0, l.width) {
        return Err(format!(
            "ground: runs from x = {} m to x = {} m, and it runs from the \
             left edge, nought, to the right, {} m",
            first.0, last.0, l.width
        ));
    }
    let steepest = STEEPEST.to_radians().tan();
    for pair in l.ground.windows(2) {
        let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
        if x1 <= x0 {
            return Err(format!(
                "ground: {x1},{y1} is not to the right of {x0},{y0}, and \
                 each point is further right than the one before it"
            ));
        }
        if (y1 - y0).abs() > steepest * (x1 - x0) * (1.0 + EQUAL_WITHIN) {
            return Err(format!(
                "ground: from {x0},{y0} to {x1},{y1} is steeper than \
                 {STEEPEST} degrees"
            ));
        }
    }
    let top = HIGHEST_GROUND * l.height;
    if let Some(&(x, y)) = l
        .ground
        .iter()
        .find(|&&(_, y)| y < 0.0 || y > top * (1.0 + EQUAL_WITHIN))
    {
        return Err(format!(
            "ground: {x},{y} is below nought or higher than {top} m, 60% of \
             the height"
        ));
    }
    let ((x0, y0), (x1, y1)) = l.pad_ends();
    if !equal(y0, y1) {
        return Err(format!("pad: from {x0},{y0} to {x1},{y1} is not level"));
    }
    let (least_x, most_x, _, _) = l.extent();
    let lander = most_x - least_x;
    if !equal(x1 - x0, PAD_TO_LANDER * lander) {
        return Err(format!(
            "pad: is {} m wide, and it is twice the lander's width, {} m",
            x1 - x0,
            PAD_TO_LANDER * lander
        ));
    }
    let middle = (x0 + x1) / 2.0;
    if middle < l.width / 4.0 || middle > 3.0 * l.width / 4.0 {
        return Err(format!(
            "pad: its middle, {middle} m, is not within the middle half of \
             the width, {} m to {} m",
            l.width / 4.0,
            3.0 * l.width / 4.0
        ));
    }
    if y0 > HIGHEST_PAD * l.height * (1.0 + EQUAL_WITHIN) {
        return Err(format!(
            "pad: at {y0} m, is higher than {} m, a fifth of the height",
            HIGHEST_PAD * l.height
        ));
    }
    let (least_x, most_x, least_y, most_y) = l.extent();
    let (x, y) = l.start;
    if x + least_x < 0.0
        || x + most_x > l.width
        || y + least_y < 0.0
        || y + most_y > l.height
    {
        return Err(format!(
            "x, y: the lander, starting at {x},{y}, is not wholly within the \
             display"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lander as the lander example states it.
    ///
    /// # Returns
    ///
    /// It.
    pub(crate) fn example() -> Lander {
        Lander {
            name: "lander".to_string(),
            dry_mass: "6000 kg".to_string(),
            fuel: "2000 kg".to_string(),
            isp: "311 s".to_string(),
            thrust_x: "12000 N".to_string(),
            thrust_y: "24000 N".to_string(),
            gravity: "1.62 m/s2".to_string(),
            x: "60 m".to_string(),
            y: "540 m".to_string(),
            width: "1000 m".to_string(),
            height: "600 m".to_string(),
            ground: "0,200 200,300 470,90 530,90 800,250 1000,150 m"
                .to_string(),
            pad: "470,90 530,90 m".to_string(),
            outline: "-15,0 15,0 15,30 -15,30 -15,0 m".to_string(),
            landing_speed: "3.05 m/s".to_string(),
            landing_drift: "1.22 m/s".to_string(),
            pause: "3 s".to_string(),
        }
    }

    /// A lander and its landscape read as stated.
    #[test]
    fn a_lander_reads() {
        let l = example().resolve().expect("it reads");
        assert_eq!(l.ground.len(), 6);
        assert_eq!(l.pad, 2);
        assert_eq!(l.pad_ends(), ((470.0, 90.0), (530.0, 90.0)));
        assert_eq!(l.extent(), (-15.0, 15.0, 0.0, 30.0));
        assert_eq!(l.start, (60.0, 540.0));
    }

    /// Each rule of the landscape is held to.
    #[test]
    fn the_landscape_keeps_its_rules() {
        let refused = |change: &dyn Fn(&mut Lander), says: &str| {
            let mut l = example();
            change(&mut l);
            let why = l.resolve().expect_err("refused");
            assert!(why.contains(says), "{why}");
        };
        refused(
            &|l| l.ground = "0,200 470,90 530,90 990,150 m".to_string(),
            "to the right, 1000 m",
        );
        refused(
            &|l| {
                l.ground = "0,200 100,500 470,90 530,90 1000,150 m".to_string()
            },
            "steeper than",
        );
        refused(
            &|l| {
                l.ground = "0,200 200,370 470,90 530,90 1000,150 m".to_string()
            },
            "higher than 360 m",
        );
        refused(&|l| l.pad = "200,300 470,90 m".to_string(), "not level");
        refused(
            &|l| {
                l.ground = "0,200 200,300 460,90 540,90 800,250 1000,150 m"
                    .to_string();
                l.pad = "460,90 540,90 m".to_string();
            },
            "twice the lander's width",
        );
        refused(
            &|l| {
                l.ground = "0,90 60,90 300,200 1000,150 m".to_string();
                l.pad = "0,90 60,90 m".to_string();
            },
            "middle half",
        );
        refused(
            &|l| {
                l.ground = "0,200 470,150 530,150 1000,150 m".to_string();
                l.pad = "470,150 530,150 m".to_string();
            },
            "a fifth of the height",
        );
        refused(&|l| l.pad = "530,90 800,250 m".to_string(), "not level");
        refused(&|l| l.pad = "100,90 160,90 m".to_string(), "not two points");
        refused(&|l| l.y = "590 m".to_string(), "wholly within");
        refused(&|l| l.ground = "0,200 1000,150".to_string(), "its unit");
        refused(&|l| l.outline = "-15,0 15 m".to_string(), "not a point");
    }
}
