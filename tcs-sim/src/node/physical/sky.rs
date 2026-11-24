//! The lander's sky: its stars, placed above its ground and as bright as
//! the night sky has them, and the records its `sky` output sends them
//! down in.
//!
//! The one place the stars are made: tcs-sim's lander pane draws them, and
//! the lander sends them, so that a ground station that draws what it is
//! sent shows the sky the pane does. See `tcsprotocol`'s
//! `node::physical::lander`, "Sky", for the records.

use tcsprotocol::node::physical::lander::{Point, Resolved, StarRecord};

/// How many stars the sky has.
pub const STARS: usize = 300;

/// The faintest star, as an apparent magnitude: about what the eye sees on a
/// dark night. The brightest is nought.
pub const FAINTEST: f64 = 6.0;

/// How many times as many stars there are at each apparent magnitude as at
/// the one brighter: about three, in the night sky.
pub const MORE_EACH_MAGNITUDE: f64 = 3.0;

/// The apparent magnitude at which a star has shrunk to a single pixel:
/// brighter ones are disks, fainter ones single pixels, dimmer the fainter.
pub const PIXEL_FROM: f64 = 3.0;

/// The radius of the brightest star's disk, and of a single pixel's, in
/// pixels.
pub const BRIGHTEST_RADIUS: f64 = 2.0;
pub const PIXEL_RADIUS: f64 = 0.5;

/// How many classes of brightness the stars are drawn in: half of them
/// disks, from the brightest to a single pixel, and half single pixels,
/// dimming.
pub const STAR_CLASSES: usize = 8;

/// How much brighter than their magnitudes say the faint stars are drawn,
/// so that they show on a screen; none brighter than full.
pub const STARS_BRIGHTER: f64 = 3.0;

/// How far above the ground a star is at least, as a fraction of the
/// display's height.
pub const STAR_CLEARANCE: f64 = 0.02;

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
pub fn ground_at(f: &Resolved, x: f64) -> f64 {
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
        // Three times that, as bright as a screen's black shows them, no
        // brighter than full.
        let bright = (STARS_BRIGHTER
            * 10f64.powf(-0.2 * (middle - PIXEL_FROM)))
        .min(1.0);
        (class, PIXEL_RADIUS, bright as f32)
    }
}

/// The stars in the sky, each somewhere above the ground, as their
/// apparent brightness draws them: the brightest small disks, shrinking to
/// a single pixel, and the fainter single pixels, dimmer the fainter.
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
/// Each star's place, in metres, its radius, in the scene's pixels, and its
/// brightness, from nought to one.
pub fn stars(f: &Resolved) -> Vec<(Point, f64, f32)> {
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
    let mut sky = Vec::new();
    for _ in 0..STARS {
        let x = next() * f.width;
        let lowest = ground_at(f, x) + STAR_CLEARANCE * f.height;
        let (u, height) = (next(), next());
        if lowest >= f.height {
            continue;
        }
        let y = lowest + height * (f.height - lowest);
        let (_, radius, brightness) = star_class(magnitude(u));
        sky.push(((x, y), radius, brightness));
    }
    sky
}

/// The sky as the records its `sky` output sends, in the stars' order.
///
/// # Arguments
///
/// * `f`          - The lander's figures.
/// * `generation` - The sky's generation, which every record carries.
///
/// # Returns
///
/// Each star's record, its bytes.
pub fn records(
    f: &Resolved,
    generation: u16,
) -> Vec<[u8; tcsprotocol::node::physical::lander::STAR_RECORD]> {
    let sky = stars(f);
    let total = u16::try_from(sky.len()).unwrap_or(u16::MAX);
    sky.iter()
        .zip(0..total)
        .map(|(&((x, y), radius, brightness), id)| {
            StarRecord {
                generation,
                total,
                id,
                radius,
                brightness: f64::from(brightness),
                x,
                y,
            }
            .to_bytes()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
            assert!(r1 < r0 || b1 <= b0, "{classes:?}");
        }
        assert_eq!(classes[STAR_CLASSES - 1].1, PIXEL_RADIUS);
        assert!(classes[STAR_CLASSES - 1].2 < 1.0, "{classes:?}");
    }

    /// Each star becomes one record, numbered from nought, each saying its
    /// generation and how many there are, and reading back as the star.
    #[test]
    fn the_sky_goes_down_as_its_stars() {
        let f = crate::lander_view::tests::figures();
        let sky = stars(&f);
        let said = records(&f, 7);
        assert_eq!(said.len(), sky.len());
        for (n, (bytes, &((x, y), radius, _))) in
            said.iter().zip(&sky).enumerate()
        {
            let star = StarRecord::from_bytes(bytes);
            assert_eq!(star.generation, 7);
            assert_eq!(usize::from(star.total), sky.len());
            assert_eq!(usize::from(star.id), n);
            assert!((star.x - x).abs() <= 0.005 && (star.y - y).abs() <= 0.005);
            assert!((star.radius - radius).abs() <= 0.05);
        }
    }
}
