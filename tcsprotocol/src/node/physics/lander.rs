//! A lander: a craft flown in two dimensions over a landscape, under
//! gravity, by thrust along each axis, burning the fuel it carries, until
//! it crashes. Set down gently on the pad, it stands there and can be flown
//! off again.
//!
//! It takes `throttle_x`, a ratio from minus one to one, and `throttle_y`, from
//! nought to one, which simulated payloads the vehicle drives set: positive
//! pushes it towards positive X, to the right, or positive Y, up, and a
//! negative X the other way. There is no thrust down: its engine is on its
//! bottom, and nothing on its top pushes it down. It gives out where it is, `x`
//! and `y`, in metres, the place between its feet; how fast it is going,
//! `velocity_x` and `velocity_y`, in metres a second; the `fuel` it has left,
//! as a fraction of a full tank; two decisions, `landed`, asserted while it
//! stands on the pad, and `crashed`, asserted from a crash until it starts
//! again; and how fast it is speeding up, `acceleration_x` and
//! `acceleration_y`, in metres a second a second, gravity's included and
//! nought while it stands on the pad or has crashed.
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
//! lander has landed, if all of it is above the pad, it meets nothing but
//! the pad, and it is going no faster than `landing_speed` down and
//! `landing_drift` sideways; it has crashed, otherwise. Leaving the display
//! is not coming down: the lander flies on out of sight, and has crashed
//! only if it drops below the display's bottom line, which goes on
//! indefinitely beyond its edges.
//!
//! Landed, it stands still on the pad, its engines burning fuel as its
//! throttles say, until thrust up greater than its weight lifts it off and
//! it flies on: a landing does not end a flight, only a crash does.
//! Crashed, it stays where it is until it is restarted, by its `restart`
//! input, or, with its `autorestart` input asserted, for `pause`; then it
//! starts again where it started, with a full tank. A restart starts it
//! again however it is, landed or flying too. The arithmetic is tcs-sim's
//! `physics::lander`.
//!
//! # Sky
//!
//! The lander's sky, its stars, is sent by its `sky` output, records a
//! simulated payload sends [`STARS_IN_A_PACKET`] at a time, round and
//! round, so that a ground station that misses one has it again on the next
//! pass. Each is [`STAR_RECORD`] bytes, most significant byte first: the
//! sky's generation and how many stars it has, two bytes each; the star's
//! number, from nought, two bytes; its radius, in tenths of a pixel of a
//! scene [`SKY_WIDTH`] pixels wide, and its brightness, from nought to
//! 255, a byte each; and its place across and up, in centimetres, four
//! bytes each. A ground station that has every star of a generation drives
//! `sky_quench` to that generation, and the lander stops sending until its
//! sky is made again, with a new generation.

use tcsos::time::Duration;

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
    Terminal::output(
        "acceleration_x",
        Kind::Value(Unit::MetrePerSecondSquared),
    ),
    Terminal::output(
        "acceleration_y",
        Kind::Value(Unit::MetrePerSecondSquared),
    ),
    // Starts it again at once, at its start with a full tank, each time it
    // is driven to a new value other than nought: what the ground's Restart
    // sends, a count, so that each press differs from the last.
    Terminal::input("restart", Kind::Decision),
    // While asserted, a lander that has crashed starts again by itself once
    // its pause has run; otherwise it stays down until it is restarted.
    // Nothing driving it, it is not asserted: by default a crashed lander
    // stays down. A landing is not the end of a flight, so it does nothing
    // to a landed one.
    Terminal::input("autorestart", Kind::Decision),
    // Its sky's records: see the module's "Sky".
    Terminal::output("sky", Kind::Records),
    // Driven to the generation of its sky that the ground has whole: while
    // it is the sky's own, the sky is not sent.
    Terminal::input("sky_quench", Kind::Decision),
];

/// How many bytes one of the sky's records is.
pub const STAR_RECORD: usize = 16;

/// How many of the sky's records a packet carries: none is split between
/// two.
pub const STARS_IN_A_PACKET: usize = 4;

/// How wide, in pixels, the scene is that a star's radius is given for.
pub const SKY_WIDTH: f64 = 600.0;

/// One star of the sky, as a record carries it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StarRecord {
    /// The sky's generation: a new one each time the sky is made.
    pub generation: u16,
    /// How many stars the sky has.
    pub total: u16,
    /// Which star this is, from nought.
    pub id: u16,
    /// Its radius, in pixels of a scene [`SKY_WIDTH`] wide.
    pub radius: f64,
    /// Its brightness, from nought to one.
    pub brightness: f64,
    /// Its place across, in metres.
    pub x: f64,
    /// Its place up, in metres.
    pub y: f64,
}

impl StarRecord {
    /// Its bytes, as the module's "Sky" lays them out.
    ///
    /// # Returns
    ///
    /// The record. A radius, brightness or place beyond what its field
    /// holds is held at the most it holds, none below nought.
    pub fn to_bytes(&self) -> [u8; STAR_RECORD] {
        let mut out = [0u8; STAR_RECORD];
        out[0..2].copy_from_slice(&self.generation.to_be_bytes());
        out[2..4].copy_from_slice(&self.total.to_be_bytes());
        out[4..6].copy_from_slice(&self.id.to_be_bytes());
        out[6] = (self.radius * 10.0).round().clamp(0.0, 255.0) as u8;
        out[7] = (self.brightness * 255.0).round().clamp(0.0, 255.0) as u8;
        let centimetres = |m: f64| {
            ((m * 100.0).round().clamp(0.0, f64::from(u32::MAX))) as u32
        };
        out[8..12].copy_from_slice(&centimetres(self.x).to_be_bytes());
        out[12..16].copy_from_slice(&centimetres(self.y).to_be_bytes());
        out
    }

    /// A record read back from its bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - The record's bytes.
    ///
    /// # Returns
    ///
    /// The star, to the precision its fields hold.
    pub fn from_bytes(bytes: &[u8; STAR_RECORD]) -> StarRecord {
        let two = |at: usize| u16::from_be_bytes([bytes[at], bytes[at + 1]]);
        let four = |at: usize| {
            f64::from(u32::from_be_bytes([
                bytes[at],
                bytes[at + 1],
                bytes[at + 2],
                bytes[at + 3],
            ]))
        };
        StarRecord {
            generation: two(0),
            total: two(2),
            id: two(4),
            radius: f64::from(bytes[6]) / 10.0,
            brightness: f64::from(bytes[7]) / 255.0,
            x: four(8) / 100.0,
            y: four(12) / 100.0,
        }
    }
}

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
    /// How long it stays down after a crash before it starts again, with
    /// autorestart asserted: `"3 s"`.
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

    /// A star's record reads back as it was written, to its fields'
    /// precision, laid out as the module's "Sky" says.
    #[test]
    fn a_star_record_reads_back() {
        let star = StarRecord {
            generation: 3,
            total: 300,
            id: 299,
            radius: 1.5,
            brightness: 0.6,
            x: 682.65,
            y: 389.97,
        };
        let bytes = star.to_bytes();
        assert_eq!(&bytes[..8], &[0, 3, 1, 44, 1, 43, 15, 153]);
        assert_eq!(&bytes[8..12], &68265u32.to_be_bytes());
        let back = StarRecord::from_bytes(&bytes);
        assert_eq!((back.generation, back.total, back.id), (3, 300, 299));
        assert_eq!((back.radius, back.x, back.y), (1.5, 682.65, 389.97));
        assert!((back.brightness - 0.6).abs() < 0.5 / 255.0);
        // Beyond its fields, held at their ends.
        let wild = StarRecord {
            radius: 40.0,
            brightness: 2.0,
            x: -1.0,
            ..star
        };
        let bytes = wild.to_bytes();
        assert_eq!((bytes[6], bytes[7]), (255, 255));
        assert_eq!(&bytes[8..12], &[0, 0, 0, 0]);
    }

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
