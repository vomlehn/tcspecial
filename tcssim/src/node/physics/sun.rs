//! The sun: one in sunlight and nought in shadow, through an orbit.
//!
//! Its terminal is `tcslibgs::node::physics::sun`'s: `light`, a ratio.
//!
//! The light changes all at once, twice an orbit, and between the two it
//! does not change at all. So the sun is a component of events: it knows when
//! it next goes into shadow or comes out, says so as its next event, and
//! changes when the world reaches it. Nothing is stepped across, and the
//! moment of each change is exact however seldom the world is asked about.

use tcslibgs::node::physics::sun::Resolved;

use super::component::Component;

/// Its terminal's place, in `tcslibgs::node::physics::sun::TERMINALS`'s order,
/// which a test holds this to.
const LIGHT: usize = 0;

/// The sun of the world.
#[derive(Debug)]
pub struct Sun {
    /// How long the sunlit part of an orbit lasts, in seconds.
    sunlit: f64,
    /// How long the shadowed part lasts, in seconds; nought for an orbit
    /// always in sunlight, which has no events.
    shadowed: f64,
    /// Whether it is in sunlight.
    lit: bool,
    /// When, in simulated seconds, it next goes into shadow or comes out.
    next: f64,
}

impl Sun {
    /// Makes the sun, as far round its orbit as its figures say it starts.
    ///
    /// # Arguments
    ///
    /// * `figures` - What the model says of it.
    ///
    /// # Returns
    ///
    /// The sun at simulated time nought.
    pub fn new(figures: Resolved) -> Sun {
        let period = figures.period.as_secs_f64();
        let sunlit = (1.0 - figures.eclipse) * period;
        let at = figures.start * period;
        let lit = at < sunlit;
        Sun {
            sunlit,
            shadowed: figures.eclipse * period,
            lit,
            next: if lit { sunlit - at } else { period - at },
        }
    }
}

impl Component for Sun {
    fn hold(&mut self, _inputs: &[f64]) {}

    fn advance(&mut self, _from: f64, _to: f64) {}

    fn output(&self, terminal: usize) -> f64 {
        match terminal {
            LIGHT if self.lit => 1.0,
            _ => 0.0,
        }
    }

    fn next_event(&self, _now: f64) -> Option<f64> {
        (self.shadowed > 0.0).then_some(self.next)
    }

    fn event(&mut self, _at: f64) {
        self.lit = !self.lit;
        self.next += if self.lit { self.sunlit } else { self.shadowed };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tcslibgs::node::physics::sun::TERMINALS;

    /// The place here is the type's.
    #[test]
    fn the_terminal_is_the_types() {
        assert_eq!(TERMINALS[LIGHT].name, "light");
    }

    /// A hundred second orbit, forty in shadow: lit for sixty, dark for
    /// forty, and round again, each change foreseen.
    #[test]
    fn it_goes_into_shadow_and_out_as_its_orbit_says() {
        let mut sun = Sun::new(Resolved {
            period: Duration::from_secs(100),
            eclipse: 0.4,
            start: 0.0,
        });
        let mut seen = Vec::new();
        for _ in 0..4 {
            seen.push((sun.output(LIGHT), sun.next_event(0.0)));
            let at = sun.next_event(0.0).expect("an event");
            sun.event(at);
        }
        assert_eq!(
            seen,
            [
                (1.0, Some(60.0)),
                (0.0, Some(100.0)),
                (1.0, Some(160.0)),
                (0.0, Some(200.0))
            ]
        );
    }

    /// Started in shadow, it comes out at the end of the orbit; with no
    /// eclipse, it is lit for ever and foresees nothing.
    #[test]
    fn it_starts_where_its_orbit_says() {
        let dark = Sun::new(Resolved {
            period: Duration::from_secs(100),
            eclipse: 0.4,
            start: 0.75,
        });
        assert_eq!(dark.output(LIGHT), 0.0);
        assert_eq!(dark.next_event(0.0), Some(25.0));
        let always = Sun::new(Resolved {
            period: Duration::from_secs(100),
            eclipse: 0.0,
            start: 0.0,
        });
        assert_eq!(always.output(LIGHT), 1.0);
        assert_eq!(always.next_event(0.0), None);
    }
}
