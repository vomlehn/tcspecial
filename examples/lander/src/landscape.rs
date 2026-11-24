//! The run's landscape: made at random from a seed, keeping the rules the
//! lander's physical model holds a landscape to, and written into the run's
//! copy of the simulator file, which tcs-sim and the station's window both
//! read.
//!
//! Made as `lander_yamcs.py` makes it -- the pad first, then the ground out
//! from each of its ends -- from the same figures, but from Rust's random
//! numbers rather than Python's: a seed gives the same landscape every time
//! here, and not the one it gives the Python.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use tcsprotocol::format::ConfigFormat;
use tcsprotocol::node::physical::lander::{
    Point, Resolved, HIGHEST_GROUND, HIGHEST_PAD, PAD_TO_LANDER,
};
use tcsprotocol::node::physical::Resolution;
use tcsprotocol::sim_config::SimConfigFile;

/// The steepest the ground is made, in degrees: under the 70 the physical model
/// allows, so that a slope rounded to a tenth of a metre stays under it.
const STEEPEST_MADE: f64 = 62.0;

/// The narrowest and widest stretch of ground, as fractions of the width.
const NARROWEST: f64 = 0.04;
const WIDEST: f64 = 0.12;

/// The lowest the pad is made, as a fraction of the height.
const LOWEST_PAD: f64 = 0.02;

/// How wide a line of the simulator file is let run before it is wrapped.
const LINE: usize = 78;

/// A landscape: the ground, and the pad, two of its points.
#[derive(Debug, Clone, PartialEq)]
pub struct Landscape {
    /// The ground's points, from the left edge to the right, in metres.
    pub ground: Vec<Point>,
    /// The pad's two ends, two points of the ground one after the other.
    pub pad: (Point, Point),
}

/// Rounds to a tenth, as the file states every figure.
///
/// # Arguments
///
/// * `x` - The figure.
///
/// # Returns
///
/// It, to a tenth.
fn tenth(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

/// Makes a landscape at random from a seed, keeping the lander's rules.
///
/// The pad first, level, twice the lander's width, its middle within the
/// middle half of the width and its height within the lowest fifth; then
/// the ground out from each of its ends to the edges, a stretch at a time,
/// each toward a height chosen between nought and the highest the ground
/// may be, and no steeper than [`STEEPEST_MADE`].
///
/// # Arguments
///
/// * `seed`         - The seed: one seed, one landscape.
/// * `width`        - The display's width, in metres.
/// * `height`       - The display's height, in metres.
/// * `lander_width` - The lander's width, in metres.
///
/// # Returns
///
/// The landscape.
pub fn made(
    seed: u64,
    width: f64,
    height: f64,
    lander_width: f64,
) -> Landscape {
    let mut rng = StdRng::seed_from_u64(seed);
    let pad_width = PAD_TO_LANDER * lander_width;
    let middle = rng.gen_range(width / 4.0..=3.0 * width / 4.0);
    let left = tenth(middle - pad_width / 2.0);
    let right = left + pad_width;
    let level =
        tenth(rng.gen_range(LOWEST_PAD * height..=HIGHEST_PAD * height));
    let steep = STEEPEST_MADE.to_radians().tan();
    let top = HIGHEST_GROUND * height;
    let (narrowest, widest) = (NARROWEST * width, WIDEST * width);

    let mut out_to = |from: Point, edge: f64| {
        let (mut x, mut y) = from;
        let mut made = Vec::new();
        while x != edge {
            let step = rng.gen_range(narrowest..=widest);
            // The last stretch reaches the edge, and is no narrower than the
            // narrowest.
            let after = if (edge - x).abs() - step < narrowest {
                edge
            } else {
                tenth(x + step.copysign(edge - x))
            };
            // Toward a height anywhere from nought to the highest, as far as
            // the slope allows.
            let most = steep * (after - x).abs();
            let aim = rng.gen_range(0.0..=top);
            x = after;
            y = tenth((y + most).min((y - most).max(aim)));
            made.push((x, y));
        }
        made
    };
    let leftward = out_to((left, level), 0.0);
    let rightward = out_to((right, level), width);
    let pad = ((left, level), (right, level));
    let mut ground: Vec<Point> = leftward.into_iter().rev().collect();
    ground.extend([pad.0, pad.1]);
    ground.extend(rightward);
    Landscape { ground, pad }
}

/// A seed for a run that was given none: from the clock.
///
/// # Returns
///
/// The seed, small enough to say and to type back.
pub fn any_seed() -> u64 {
    tcsos::time::SystemTime::now()
        .duration_since(tcsos::time::UNIX_EPOCH)
        // Milliseconds, which a page's clock counts in as well.
        .map(|since| (since.as_millis() % 1_000_000) as u64)
        .unwrap_or(1)
}

/// Says a line of points as the simulator file states them.
///
/// # Arguments
///
/// * `points` - Each `(x, y)`, in metres.
///
/// # Returns
///
/// The points, `"x,y x,y ... m"`.
pub fn said_of(points: &[Point]) -> String {
    let mut said: Vec<String> =
        points.iter().map(|(x, y)| format!("{x},{y}")).collect();
    said.push("m".to_string());
    said.join(" ")
}

/// The lander the simulator file states, its figures read.
///
/// # Arguments
///
/// * `sim_text` - The file's text.
///
/// # Returns
///
/// Its figures.
///
/// # Errors
///
/// Where the file does not read, or has no lander, or more than one.
pub fn lander_of(sim_text: &str) -> Result<Resolved, String> {
    let file = SimConfigFile::parse(sim_text, ConfigFormat::Yaml)
        .map_err(|e| e.to_string())?;
    let model = file
        .physical()
        .map_err(|e| e.to_string())?
        .ok_or("the simulator file has no physical model")?;
    let landers: Vec<Resolved> = model
        .components
        .into_iter()
        .filter_map(|c| match c.what {
            Resolution::Lander(figures) => Some(figures),
            _ => None,
        })
        .collect();
    match <[Resolved; 1]>::try_from(landers) {
        Ok([lander]) => Ok(lander),
        Err(found) => Err(format!(
            "the simulator file has {} landers, and this station watches one",
            found.len()
        )),
    }
}

/// Replaces one setting's quoted value, which may run over several lines,
/// where the file states it once.
///
/// # Arguments
///
/// * `text`  - The file's text.
/// * `key`   - The setting.
/// * `value` - Its new value, unquoted.
///
/// # Returns
///
/// The text, the value wrapped to the file's width under the setting.
///
/// # Errors
///
/// Where the file does not state the setting exactly once.
fn replaced(text: &str, key: &str, value: &str) -> Result<String, String> {
    let opening = format!("{key}: \"");
    let starts: Vec<usize> = text
        .match_indices(&opening)
        .map(|(at, _)| at)
        .filter(|&at| {
            let line = text[..at].rfind('\n').map_or(0, |n| n + 1);
            text[line..at].chars().all(|c| c == ' ')
        })
        .collect();
    let [at] = starts[..] else {
        return Err(format!(
            "the simulator file states {key} {} times, and it is replaced \
             once",
            starts.len()
        ));
    };
    let line = text[..at].rfind('\n').map_or(0, |n| n + 1);
    let indent = &text[line..at];
    let open = at + opening.len();
    let close = open
        + text[open..]
            .find('"')
            .ok_or_else(|| format!("{key}: its quotation is never closed"))?;
    let mut lines = vec![format!("{indent}{key}: \"")];
    let mut first = true;
    for word in value.split(' ') {
        let last = lines.last_mut().expect("a line");
        let room = LINE.saturating_sub(last.len() + 1);
        if first {
            last.push_str(word);
            first = false;
        } else if word.len() < room {
            last.push(' ');
            last.push_str(word);
        } else {
            lines.push(format!("{indent}  {word}"));
        }
    }
    lines.last_mut().expect("a line").push('"');
    Ok(format!(
        "{}{}{}",
        &text[..line],
        lines.join("\n"),
        &text[close + 1..]
    ))
}

/// The run's simulator file: the shipped one with its landscape replaced,
/// everything else, its comments too, left as it is.
///
/// # Arguments
///
/// * `shipped`   - The shipped file's text.
/// * `seed`      - The landscape's seed, which the file says.
/// * `landscape` - The landscape.
///
/// # Returns
///
/// The run's file's text.
///
/// # Errors
///
/// Where the file does not state the ground and the pad once each.
pub fn run_sim_text(
    shipped: &str,
    seed: u64,
    landscape: &Landscape,
) -> Result<String, String> {
    let text = replaced(shipped, "ground", &said_of(&landscape.ground))?;
    let (left, right) = landscape.pad;
    let text = replaced(&text, "pad", &said_of(&[left, right]))?;
    Ok(format!(
        "# Written by lander-station from lander-sim.yaml, its landscape \
         made\n# from seed {seed}.\n\n{text}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped simulator file.
    const SHIPPED: &str = include_str!("../../lander-common/lander-sim.yaml");

    /// The shipped lander's width, from its outline.
    ///
    /// # Returns
    ///
    /// The width, in metres.
    fn lander_width() -> f64 {
        let lander = lander_of(SHIPPED).expect("the shipped file reads");
        let (least, most, _, _) = lander.extent();
        most - least
    }

    /// Every landscape made keeps the rules the physical model holds one to:
    /// the run's file, made from it, is read and checked as tcs-sim reads and
    /// checks it.
    #[test]
    fn every_landscape_made_keeps_the_rules() {
        let width = lander_width();
        for seed in 0..300 {
            let landscape = made(seed, 1000.0, 600.0, width);
            let text = run_sim_text(SHIPPED, seed, &landscape)
                .expect("the shipped file states each once");
            let lander = lander_of(&text)
                .unwrap_or_else(|e| panic!("seed {seed}: {e}\n{text}"));
            assert_eq!(lander.ground, landscape.ground, "seed {seed}");
            assert_eq!(lander.pad_ends(), landscape.pad, "seed {seed}");
        }
    }

    /// One seed, one landscape; another seed, another.
    #[test]
    fn a_seed_makes_its_own_landscape() {
        let width = lander_width();
        assert_eq!(
            made(7, 1000.0, 600.0, width),
            made(7, 1000.0, 600.0, width)
        );
        assert_ne!(
            made(7, 1000.0, 600.0, width),
            made(8, 1000.0, 600.0, width)
        );
    }

    /// The rest of the file is left as it was, and no line written is wider
    /// than the file's width.
    #[test]
    fn only_the_landscape_is_replaced() {
        let landscape = made(3, 1000.0, 600.0, lander_width());
        let text = run_sim_text(SHIPPED, 3, &landscape).expect("replaced");
        assert!(text.starts_with("# Written by lander-station"));
        assert!(text.contains("from seed 3."));
        let outline = SHIPPED
            .lines()
            .find(|line| line.contains("outline:"))
            .expect("an outline");
        assert!(text.contains(outline));
        let shipped: Vec<&str> = SHIPPED.lines().collect();
        assert!(
            text.lines()
                .filter(|line| !shipped.contains(line))
                .all(|line| line.len() <= LINE),
            "{text}"
        );
    }

    /// A setting stated twice, or not at all, is refused.
    #[test]
    fn a_setting_is_replaced_only_where_it_is_stated_once() {
        assert!(replaced("a: \"1\"\n", "ground", "x").is_err());
        assert!(
            replaced("ground: \"1\"\nground: \"2\"\n", "ground", "x").is_err()
        );
        assert_eq!(
            replaced("  ground: \"1\n    2\"\nb: 3\n", "ground", "x y m"),
            Ok("  ground: \"x y m\"\nb: 3\n".to_string())
        );
    }
}
