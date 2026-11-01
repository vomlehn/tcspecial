//! A solar array: current in proportion to the light on it.
//!
//! Its terminals are `tcslibgs::physics::solar_array`'s: `light`, a ratio,
//! and `current`, in amperes, its full-sun current times the light.

use tcslibgs::physics::solar_array::Resolved;

use super::component::Component;

/// Its terminals' places, in `tcslibgs::physics::solar_array::TERMINALS`'s
/// order, which a test holds this to.
const LIGHT: usize = 0;
const CURRENT: usize = 1;

/// A solar array of the world.
#[derive(Debug)]
pub struct SolarArray {
    figures: Resolved,
    /// The light on it, as last held.
    light: f64,
}

impl SolarArray {
    /// Makes an array in the dark.
    ///
    /// # Arguments
    ///
    /// * `figures` - What the model says of it.
    ///
    /// # Returns
    ///
    /// The array.
    pub fn new(figures: Resolved) -> SolarArray {
        SolarArray {
            figures,
            light: 0.0,
        }
    }
}

impl Component for SolarArray {
    fn hold(&mut self, inputs: &[f64]) {
        self.light = inputs[LIGHT];
    }

    fn advance(&mut self, _from: f64, _to: f64) {}

    fn output(&self, terminal: usize) -> f64 {
        match terminal {
            CURRENT => self.figures.current * self.light,
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcslibgs::physics::solar_array::TERMINALS;

    /// The places here are the type's, and its current goes with its light.
    #[test]
    fn its_current_goes_with_its_light() {
        assert_eq!(TERMINALS[LIGHT].name, "light");
        assert_eq!(TERMINALS[CURRENT].name, "current");
        let mut array = SolarArray::new(Resolved { current: 7.0 });
        assert_eq!(array.output(CURRENT), 0.0);
        array.hold(&[1.0, 0.0]);
        assert_eq!(array.output(CURRENT), 7.0);
        array.hold(&[0.5, 0.0]);
        assert_eq!(array.output(CURRENT), 3.5);
    }
}
