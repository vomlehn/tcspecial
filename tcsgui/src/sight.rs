//! What the window's colours look like to a reader who cannot see all of
//! them.
//!
//! Here so that "nothing is told apart by colour alone" has a companion the
//! tests can check: the rule in `style.rs` says colour is never the only
//! channel, and this says the colours are worth having anyway. One tint per
//! node type is only useful if a reader with a dichromacy gets fifteen
//! distinguishable tints rather than nine and a few coincidences, and that is
//! not something a person can check by looking -- which is exactly the sort of
//! claim that belongs in a test.
//!
//! The three dichromacies are simulated by the usual route: sRGB to linear
//! light, through a matrix that collapses the missing cone's contribution, and
//! back. The matrices are Viénot, Brettel and Mollon's, which is the method
//! every colour blindness simulator in common use is built on. Differences are
//! measured in CIE L\*a\*b\*, where a distance means roughly the same amount of
//! "looks different" wherever in the space it is taken -- which plain RGB
//! distance does not.
//!
//! Test-only, and deliberately so: the window needs none of this at run time.
//! It is the check that the table in `main.rs` is what it claims to be.

/// A way of seeing colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sight {
    /// All three kinds of cone.
    Normal,
    /// No long-wavelength cone. About one man in a hundred.
    Protanopia,
    /// No medium-wavelength cone. About one man in sixty, and the commonest.
    Deuteranopia,
    /// No short-wavelength cone. Rare, and the one a palette chosen for the
    /// other two can still fail.
    Tritanopia,
}

impl Sight {
    /// Every way of seeing a palette has to survive.
    pub const ALL: [Sight; 4] = [
        Sight::Normal,
        Sight::Protanopia,
        Sight::Deuteranopia,
        Sight::Tritanopia,
    ];
}

/// One channel of an sRGB colour as linear light.
fn linear(channel: u8) -> f64 {
    let c = channel as f64 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// And back, clamped: a simulation can land a little outside the gamut, and a
/// colour outside it is shown as the nearest one that is in it -- which is
/// what a screen does too.
fn encoded(light: f64) -> f64 {
    let c = light.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn through(matrix: [[f64; 3]; 3], rgb: (u8, u8, u8)) -> (u8, u8, u8) {
    let v = [linear(rgb.0), linear(rgb.1), linear(rgb.2)];
    let out: Vec<f64> = (0..3)
        .map(|i| (0..3).map(|j| matrix[i][j] * v[j]).sum())
        .collect();
    (
        (encoded(out[0]) * 255.0).round() as u8,
        (encoded(out[1]) * 255.0).round() as u8,
        (encoded(out[2]) * 255.0).round() as u8,
    )
}

/// `rgb` as this way of seeing shows it.
pub fn seen(rgb: (u8, u8, u8), sight: Sight) -> (u8, u8, u8) {
    match sight {
        Sight::Normal => rgb,
        Sight::Protanopia => through(
            [
                [0.1121, 0.8853, -0.0005],
                [0.1127, 0.8897, -0.0001],
                [0.0045, 0.0000, 1.0000],
            ],
            rgb,
        ),
        Sight::Deuteranopia => through(
            [
                [0.2920, 0.7054, -0.0003],
                [0.2934, 0.7089, 0.0000],
                [-0.0209, 0.0272, 1.0000],
            ],
            rgb,
        ),
        Sight::Tritanopia => through(
            [
                [1.0000, 0.1502, -0.1516],
                [0.0000, 0.8529, 0.1478],
                [0.0000, 0.1273, 0.8723],
            ],
            rgb,
        ),
    }
}

/// `rgb` in CIE L\*a\*b\*, under a D65 white.
pub fn lab(rgb: (u8, u8, u8)) -> (f64, f64, f64) {
    let (r, g, b) = (linear(rgb.0), linear(rgb.1), linear(rgb.2));
    let x = 0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b;
    let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b;
    let z = 0.019_333_9 * r + 0.119_192_0 * g + 0.950_304_1 * b;

    let f = |t: f64| {
        if t > 216.0 / 24389.0 {
            t.cbrt()
        } else {
            (841.0 / 108.0) * t + 4.0 / 29.0
        }
    };
    let (fx, fy, fz) = (f(x / 0.950_47), f(y), f(z / 1.088_83));
    (116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz))
}

/// How different two colours look, in CIE units.
///
/// Ten is "clearly two colours" and two or three is "the same colour in two
/// printings". The plain 1976 distance rather than one of its successors: the
/// successors are corrections for pairs that are already close, and what is
/// wanted here is a floor well above that.
pub fn difference(a: (u8, u8, u8), b: (u8, u8, u8)) -> f64 {
    let (one, two) = (lab(a), lab(b));
    ((one.0 - two.0).powi(2) + (one.1 - two.1).powi(2) + (one.2 - two.2).powi(2)).sqrt()
}

/// The contrast ratio of `rgb` against white, as the accessibility guidelines
/// define it.
///
/// Three to one is the threshold for something that is not text -- a border, a
/// line, a shape -- and the canvas is white.
pub fn contrast_on_white(rgb: (u8, u8, u8)) -> f64 {
    let light =
        0.2126 * linear(rgb.0) + 0.7152 * linear(rgb.1) + 0.0722 * linear(rgb.2);
    1.05 / (light + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grey is a grey to everybody.
    ///
    /// The simulation's one invariant worth stating: a colour with no chroma
    /// has nothing for a missing cone to miss, so a simulation that moved one
    /// would be a simulation with its matrices transposed or its gamma
    /// forgotten -- which is how this sort of code fails, and it fails
    /// quietly.
    #[test]
    fn a_grey_looks_the_same_to_everybody() {
        for level in [0u8, 64, 128, 192, 255] {
            let grey = (level, level, level);
            for sight in Sight::ALL {
                let shown = seen(grey, sight);
                assert!(
                    difference(grey, shown) < 3.0,
                    "{sight:?} turns grey {level} into {shown:?}"
                );
            }
        }
    }

    /// Red and green are the same colour to most colour blind readers.
    ///
    /// The other half of the check: a simulation that left everything alone
    /// would pass the test above and prove nothing. This is the confusion the
    /// whole exercise is about, so the simulation had better show it.
    #[test]
    fn red_and_green_run_together_without_the_cones_for_them() {
        // Of a matched lightness, which is what makes them a confusion pair:
        // a dark red and a light green stay apart for anybody, because
        // lightness is not what either of these is missing.
        let red = (0xa8, 0x4d, 0x20);
        let green = (0x55, 0x7a, 0x2a);
        assert!(
            difference(red, green) > 40.0,
            "red and green are plainly different to begin with"
        );
        for sight in [Sight::Protanopia, Sight::Deuteranopia] {
            let apart = difference(seen(red, sight), seen(green, sight));
            assert!(
                apart < 20.0,
                "{sight:?} should struggle to tell red from green, and sees \
                 them {apart:.1} apart"
            );
        }
    }

    /// White is white and black is black, whatever is missing.
    #[test]
    fn the_ends_of_the_scale_stay_put() {
        for sight in Sight::ALL {
            assert!(seen((255, 255, 255), sight).0 > 250, "{sight:?}");
            assert!(seen((0, 0, 0), sight).1 < 5, "{sight:?}");
        }
        assert!(contrast_on_white((255, 255, 255)) < 1.05);
        assert!(contrast_on_white((0, 0, 0)) > 20.0);
    }
}

